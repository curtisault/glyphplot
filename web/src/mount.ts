// `mount(el, chart, { input })`: draws a chart into a `<pre>` (text) or a
// `<canvas>` and, with `input`, turns pointer, wheel, touch and key events
// into `chart.input` calls and dispatches the chart's events
// (REQ-MOUNT-010). It draws at most once a display frame and only when
// something changed (REQ-PERF-020), only while the drawing intersects the
// viewport and the document is visible, and advances play time only while a
// `motion` record is in flight (REQ-MOUNT-020). Input is taken from Pointer
// Events only (REQ-TOUCH-020), coalesced to one move a frame
// (REQ-INPUT-030), with the defaults of REQ-INPUT-040 and REQ-TOUCH-010.

import { Changed, InputKind, Token, type WasmEvent } from "./api.ts";
import type { Change, Chart, View } from "./chart.ts";
import { EventGate, WHEEL_IDLE_MS, type GestureEnd } from "./events.ts";
import { Gestures, InputQueue, keyCode, modsOfEvent, type Encoded, type PointerSample } from "./input.ts";
import { makeRenderer, type DrawStats, type RendererName } from "./render.ts";
import { paletteCss } from "./theme.ts";
import { messageOf, type GridViews } from "./wasm.ts";

/** How mount draws and listens. */
export interface MountOptions {
  /** Listen to pointer, wheel, touch and key events. */
  input?: boolean;
  /** The canvas renderer (REQ-RENDER-050); default `text`. */
  renderer?: RendererName;
  /** Where events are dispatched; default the drawing element. */
  target?: EventTarget;
  /** The element pointer and wheel listeners go on; default the drawing element. */
  surface?: HTMLElement;
  /** The element that takes focus and keys; default the drawing element. */
  keys?: HTMLElement;
  /** Frames drawn on paper. */
  paper?: boolean;
  /** Holds play time at this value (the `time` attribute). */
  time?: number | null;
  /** Let view animations play under `prefers-reduced-motion: reduce` (the `motion` attribute). */
  motion?: boolean;
  /** The palette (1,020 bytes) and whether to paint its ground. */
  palette?: Uint8Array;
  paintGround?: boolean;
  /** Draw nothing until the first `resize` (the element sizes the grid first). */
  waitForSize?: boolean;
  /** Called after each frame drawn, with the grid and what was redrawn. */
  onFrame?: (g: GridViews, stats: DrawStats | null) => void;
  /** Called with each dispatched event. */
  onEvent?: (e: WasmEvent) => void;
  /** Called when a call traps or fails; drawing stops. */
  onError?: (message: string) => void;
}

/** A mounted chart. */
export interface Mount {
  /** Removes every listener and observer and cancels the frame loop. */
  stop(): void;
  /** A new grid size, laid out and drawn now when visible (REQ-EL-090); `cw` the cell width in device px. */
  resize(cols: number, rows: number, cw: number): void;
  /** A new palette, repainted from the last frame without drawing a new one (REQ-THEME-030). */
  setPalette(bytes: Uint8Array, paintGround: boolean): void;
  /** Frames on paper or not. */
  setPaper(on: boolean): void;
  /** Holds play time (null lets it run). */
  setTime(t: number | null): void;
  /** Lets animations play under reduced motion. */
  setMotion(on: boolean): void;
  /** Redraws every cell from the last frame (a font load). */
  repaint(): void;
  /** Marks the chart changed: the next display frame draws it. */
  invalidate(): void;
  /** Sends one input event as the reader's, as the control row does. */
  send(e: Encoded, end?: GestureEnd): void;
  /** Play time in seconds. */
  readonly t: number;
  /** `frame` calls made: a theme change makes none. */
  readonly frames: number;
  readonly cols: number;
  readonly rows: number;
}

const hasDom = (): boolean => typeof document !== "undefined";
const raf = (f: (now: number) => void): number => (typeof requestAnimationFrame === "function" ? requestAnimationFrame(f) : (setTimeout(() => f(performance.now()), 16) as unknown as number));
const caf = (h: number): void => (typeof cancelAnimationFrame === "function" ? cancelAnimationFrame(h) : clearTimeout(h));

/** Whether the reader asked for reduced motion. */
export function prefersReducedMotion(): boolean {
  return typeof matchMedia === "function" && matchMedia("(prefers-reduced-motion: reduce)").matches;
}

/** Mounts `chart`, which must have loaded, on `el`: a `<canvas>` or a `<pre>`. */
export function mount(el: HTMLCanvasElement | HTMLPreElement, chart: Chart, opts: MountOptions = {}): Mount {
  const isCanvas = typeof HTMLCanvasElement !== "undefined" && el instanceof HTMLCanvasElement;
  const renderer = isCanvas ? makeRenderer(opts.renderer ?? "text", el as HTMLCanvasElement) : null;
  const target = opts.target ?? el;
  const surface = opts.surface ?? el;
  const keys = opts.keys ?? el;
  const gate = new EventGate();
  const queue = new InputQueue();
  const stops: (() => void)[] = [];
  let stopped = false;
  let rafId: number | null = null;
  let dirty = true;
  let visible = !hasDom() || typeof IntersectionObserver !== "function";
  let docVisible = !hasDom() || document.visibilityState !== "hidden";
  let t = opts.time ?? 0;
  let fixedTime = opts.time ?? null;
  let lastNow: number | null = null;
  let motionEnd: number | null = null;
  let checkMotion = false;
  let paper = opts.paper ?? false;
  let allowMotion = opts.motion ?? false;
  let frames = 0;
  let lastText = "";
  let ended: GestureEnd[] = [];
  let wheelTimer: ReturnType<typeof setTimeout> | null = null;
  let lastEscape = false;
  let cols = 0;
  let rows = 0;
  let sized = !opts.waitForSize;

  const fail = (e: unknown): void => {
    const m = messageOf(e);
    stop();
    if (opts.onError) opts.onError(m);
    else console.error(`glyphplot: drawing stopped: ${m}`);
  };

  const dispatch = (e: WasmEvent): void => {
    opts.onEvent?.(e);
    if (typeof CustomEvent === "function") target.dispatchEvent(new CustomEvent(e.type, { detail: e.detail, bubbles: true, composed: true }));
  };

  const viewNow = (): View | null => {
    try {
      return chart.view;
    } catch {
      return null;
    }
  };

  const schedule = (): void => {
    if (stopped || rafId !== null || !visible || !docVisible || !sized) return;
    if (!dirty && !queue.pending && motionEnd === null && ended.length === 0) return;
    rafId = raf(tick);
  };

  /** Reads the view for a motion record; under reduced motion without `motion`, jumps to its end (REQ-A11Y-050, REQ-VIEW-050). */
  const readMotion = (): void => {
    checkMotion = false;
    const v = viewNow();
    const m = v?.["motion"] as { t0: number; duration: number; to: [number, number] } | null | undefined;
    if (!m || m.t0 + m.duration <= t) {
      motionEnd = null; // none, or one that has run its course
      return;
    }
    if (!allowMotion && prefersReducedMotion()) {
      chart.view = { x: { domain: m.to }, motion: null };
      motionEnd = null;
      dirty = true;
      return;
    }
    motionEnd = m.t0 + m.duration;
  };

  const draw = (): void => {
    const g = chart.frame(t, paper);
    frames++;
    let stats: DrawStats | null = null;
    if (renderer) stats = renderer.draw(g);
    else {
      const text = chart.text();
      if (text !== lastText) {
        el.textContent = text;
        lastText = text;
      }
    }
    cols = g.cols;
    rows = g.rows;
    dirty = false;
    opts.onFrame?.(g, stats);
  };

  const closeGestures = (): void => {
    if (ended.length === 0) return;
    const ends = ended;
    ended = [];
    for (const end of ends) {
      if (!gate.open) break;
      const e = gate.close(end, viewNow());
      if (e) dispatch(e);
    }
  };

  function tick(now: number): void {
    rafId = null;
    if (stopped) return;
    try {
      if (fixedTime !== null) t = fixedTime;
      else if (motionEnd !== null) {
        if (lastNow !== null) t += Math.max(0, now - lastNow) / 1000;
        lastNow = now;
      } else lastNow = null;
      let changed = 0;
      for (const e of queue.take()) changed |= chart.inputRaw(e.kind, e.x, e.y, e.a, e.b, e.buttons, e.mods, t);
      if (changed & Changed.events) {
        for (const e of gate.frame(chart.events())) {
          dispatch(e);
          const d = e.detail as { source?: string; final?: boolean } | null;
          if (e.type === "plot-view" && d && d.final === false && d.source === "wheel") armWheel();
        }
      }
      if (changed & (Changed.overlay | Changed.base)) dirty = true;
      if (changed & Changed.base) checkMotion = true;
      if (checkMotion) readMotion();
      const moving = motionEnd !== null;
      if (dirty || moving) draw();
      if (moving && motionEnd !== null && t >= motionEnd) {
        motionEnd = null;
        lastNow = null;
        checkMotion = true;
        dirty = true;
        ended.push("motion");
      }
      closeGestures();
    } catch (e) {
      fail(e);
      return;
    }
    schedule();
  }

  /**
   * After the wheel has rested 150 ms, a wheel of no delta: the reducer ends
   * its open zoom with the one `final: true` (REQ-EVENT-030). Ending it here
   * alone left the module's zoom open, so its next input fired a second final.
   */
  const armWheel = (): void => {
    if (wheelTimer !== null) clearTimeout(wheelTimer);
    wheelTimer = setTimeout(() => {
      wheelTimer = null;
      push({ kind: InputKind.wheel, x: lastWheel.x, y: lastWheel.y, a: 0, b: 0, buttons: 0, mods: lastWheel.mods });
      ended.push("wheel");
      schedule();
    }, WHEEL_IDLE_MS);
  };
  let lastWheel = { x: 0, y: 0, mods: 0 };

  const push = (e: Encoded): void => {
    if (stopped) return;
    queue.push(e);
    schedule();
  };

  const unsubscribe = chart.subscribe((c: Change) => {
    if (c === "view") checkMotion = true;
    if (c === "data") gate.resetHover();
    dirty = true;
    schedule();
  });
  stops.push(unsubscribe);

  if (hasDom()) {
    if (typeof IntersectionObserver === "function") {
      const io = new IntersectionObserver((entries) => {
        const last = entries[entries.length - 1];
        visible = last ? last.isIntersecting : visible;
        schedule();
      });
      io.observe(el);
      stops.push(() => io.disconnect());
    }
    const onVis = (): void => {
      docVisible = document.visibilityState !== "hidden";
      schedule();
    };
    document.addEventListener("visibilitychange", onVis);
    stops.push(() => document.removeEventListener("visibilitychange", onVis));
    const fonts = (document as Document & { fonts?: FontFaceSet }).fonts;
    if (fonts && renderer) {
      const onFonts = (): void => {
        if (stopped) return;
        renderer.repaint();
      };
      fonts.addEventListener("loadingdone", onFonts);
      stops.push(() => fonts.removeEventListener("loadingdone", onFonts));
      fonts.load(`16px "glyphplot mono"`, "0⠀").then(onFonts, () => {});
    }
  }

  if (opts.input && hasDom()) listen();

  function listen(): void {
    const on = <K extends keyof HTMLElementEventMap>(node: HTMLElement, type: K, f: (e: HTMLElementEventMap[K]) => void, o?: AddEventListenerOptions): void => {
      node.addEventListener(type, f as EventListener, o);
      stops.push(() => node.removeEventListener(type, f as EventListener, o));
    };
    const gestures = new Gestures(push);
    stops.push(() => gestures.reset());
    const sample = (e: PointerEvent): PointerSample => {
      const r = el.getBoundingClientRect();
      const px = e.clientX - r.left;
      const py = e.clientY - r.top;
      const c = cols || 1;
      const rr = rows || 1;
      return {
        id: e.pointerId,
        type: e.pointerType || "mouse",
        px,
        py,
        x: r.width > 0 ? (px / r.width) * c : 0,
        y: r.height > 0 ? (py / r.height) * rr : 0,
        buttons: e.buttons,
        mods: modsOfEvent(e),
        time: e.timeStamp,
      };
    };
    on(surface, "pointerdown", (e) => {
      if (e.pointerType === "mouse" && e.button !== 0 && e.button !== 1) return;
      try {
        surface.setPointerCapture(e.pointerId);
      } catch {
        // a synthetic pointer cannot be captured
      }
      gestures.down(sample(e));
    });
    on(surface, "pointermove", (e) => gestures.move(sample(e)));
    on(surface, "pointerup", (e) => {
      gestures.up(sample(e));
      ended.push(e.pointerType === "touch" ? "touch" : "pointer");
      schedule();
    });
    on(surface, "pointercancel", (e) => {
      gestures.up(sample(e), true);
      ended.push(e.pointerType === "touch" ? "touch" : "pointer");
      schedule();
    });
    on(surface, "pointerleave", (e) => {
      if (e.pointerType !== "touch" && e.buttons === 0) push({ kind: InputKind.leave, x: 0, y: 0, a: 0, b: 0, buttons: 0, mods: 0 });
    });
    on(
      surface,
      "wheel",
      (e) => {
        const zoom = chart.options["wheel"] === "zoom";
        if (!(e.ctrlKey || e.metaKey || zoom)) return; // the plain wheel scrolls the page (REQ-INPUT-040)
        e.preventDefault();
        const r = el.getBoundingClientRect();
        const lineCss = rows > 0 && r.height > 0 ? r.height / rows : 16;
        const scale = e.deltaMode === 1 ? 1 : e.deltaMode === 2 ? Math.max(1, rows) : 1 / lineCss;
        lastWheel = {
          x: r.width > 0 ? ((e.clientX - r.left) / r.width) * (cols || 1) : 0,
          y: r.height > 0 ? ((e.clientY - r.top) / r.height) * (rows || 1) : 0,
          mods: modsOfEvent(e),
        };
        push({ kind: InputKind.wheel, x: lastWheel.x, y: lastWheel.y, a: e.deltaX * scale, b: e.deltaY * scale, buttons: 0, mods: lastWheel.mods });
      },
      { passive: false },
    );
    on(keys, "keydown", (e) => {
      if (e.isComposing) return;
      const code = keyCode(e.key);
      if (code < 0) return;
      e.preventDefault();
      if (e.key === "Escape") {
        if (lastEscape) {
          lastEscape = false;
          keys.blur();
          return;
        }
        lastEscape = true;
      } else lastEscape = false;
      push({ kind: InputKind.key, x: 0, y: 0, a: code, b: 0, buttons: 0, mods: modsOfEvent(e) });
    });
    on(keys, "keyup", () => {
      ended.push("key");
      schedule();
    });
    on(keys, "blur", () => {
      lastEscape = false;
      push({ kind: InputKind.leave, x: 0, y: 0, a: 0, b: 0, buttons: 0, mods: 0 });
    });
  }

  if (opts.palette) setPalette(opts.palette, opts.paintGround ?? false);

  function setPalette(bytes: Uint8Array, paintGround: boolean): void {
    if (renderer) {
      renderer.setPalette(bytes, paintGround);
      renderer.repaint();
    } else {
      const css = paletteCss(bytes);
      el.style.color = css[Token.ink] ?? "";
      el.style.background = paintGround ? (css[Token.ground] ?? "") : "";
    }
  }

  function stop(): void {
    if (stopped) return;
    stopped = true;
    if (rafId !== null) caf(rafId);
    rafId = null;
    if (wheelTimer !== null) clearTimeout(wheelTimer);
    queue.clear();
    for (const s of stops.splice(0)) s();
  }

  schedule();

  return {
    stop,
    resize(c: number, r: number, cw: number) {
      if (stopped) return;
      try {
        chart.resize(c, r);
        renderer?.resize(c, r, cw);
        cols = c;
        rows = r;
        sized = true;
        dirty = true;
        if (visible && docVisible) draw(); // within this frame (REQ-EL-090)
      } catch (e) {
        fail(e);
      }
    },
    setPalette,
    setPaper(on: boolean) {
      paper = on;
      dirty = true;
      schedule();
    },
    setTime(v: number | null) {
      fixedTime = v;
      if (v !== null) t = v;
      dirty = true;
      schedule();
    },
    setMotion(on: boolean) {
      allowMotion = on;
    },
    repaint() {
      renderer?.repaint();
    },
    invalidate() {
      dirty = true;
      schedule();
    },
    send(e: Encoded, end?: GestureEnd) {
      push(e);
      if (end) ended.push(end);
    },
    get t() {
      return t;
    },
    get frames() {
      return frames;
    },
    get cols() {
      return cols;
    },
    get rows() {
      return rows;
    },
  };
}

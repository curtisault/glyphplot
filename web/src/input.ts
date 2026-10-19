// Input in the shell, without the DOM: the numeric encoding of
// input.schema.json events (REQ-INPUT-020), the per-frame queue that keeps
// the discrete events in order and the latest move last (REQ-INPUT-030), and
// the touch recogniser for scrub, two-finger pan, pinch, tap, double tap and
// long press (REQ-TOUCH-010). mount.ts feeds it DOM events.

import { InputKind, type InputKindName, KEYS, Mods, PointerCode, SHELL_KEY_COUNT } from "./api.ts";

/** One event in the boundary's numeric encoding; `t` is stamped when the queue is flushed. */
export interface Encoded {
  kind: number;
  x: number;
  y: number;
  a: number;
  b: number;
  buttons: number;
  mods: number;
}

/** An input event in the JSON form of input.schema.json. */
export interface InputEvent {
  kind: InputKindName;
  t?: number;
  x?: number;
  y?: number;
  pointer?: "mouse" | "pen" | "touch";
  buttons?: number;
  mods?: readonly ("shift" | "ctrl" | "alt" | "meta")[];
  dx?: number;
  dy?: number;
  scale?: number;
  end?: boolean;
  velocity?: number;
  count?: 1 | 2;
  key?: string;
}

/** The mods bitmask of a list of modifier names. */
export function modsOf(mods: readonly string[] | undefined): number {
  let m = 0;
  for (const k of mods ?? []) m |= (Mods as Record<string, number>)[k] ?? 0;
  return m;
}

/** The mods bitmask of a DOM event's modifier flags. */
export function modsOfEvent(e: { shiftKey: boolean; ctrlKey: boolean; altKey: boolean; metaKey: boolean }): number {
  return (e.shiftKey ? Mods.shift : 0) | (e.ctrlKey ? Mods.ctrl : 0) | (e.altKey ? Mods.alt : 0) | (e.metaKey ? Mods.meta : 0);
}

/** The key code of a `KeyboardEvent.key` the shell sends, or -1 (codes 25–30 are the CLI's). */
export function keyCode(key: string): number {
  const i = KEYS.indexOf(key);
  return i >= 0 && i < SHELL_KEY_COUNT ? i : -1;
}

/** `buttons` as the boundary takes it: the buttons bitmask plus 256 × the pointer type. */
export function buttonsOf(buttons: number, pointer: string | undefined): number {
  const p = pointer === "pen" ? PointerCode.pen : pointer === "touch" ? PointerCode.touch : PointerCode.mouse;
  return (buttons & 31) + 256 * p;
}

/** An input.schema.json event in the numeric encoding (the plan's "Input encoding" table). Throws on an unknown kind or key. */
export function encode(e: InputEvent): Encoded {
  const kind = InputKind[e.kind];
  if (kind === undefined) throw new Error(`unknown input kind \`${String(e.kind)}\``);
  const out: Encoded = { kind, x: 0, y: 0, a: 0, b: 0, buttons: 0, mods: modsOf(e.mods) };
  switch (e.kind) {
    case "move":
    case "down":
    case "up":
      out.x = e.x ?? 0;
      out.y = e.y ?? 0;
      out.buttons = buttonsOf(e.buttons ?? 0, e.pointer);
      break;
    case "leave":
      out.mods = 0;
      break;
    case "wheel":
      out.x = e.x ?? 0;
      out.y = e.y ?? 0;
      out.a = e.dx ?? 0;
      out.b = e.dy ?? 0;
      break;
    case "pinch":
      out.x = e.x ?? 0;
      out.y = e.y ?? 0;
      out.a = e.scale ?? 1;
      out.b = e.end ? 1 : 0;
      break;
    case "pan":
      out.x = e.velocity ?? 0;
      out.y = e.end ? 1 : 0;
      out.a = e.dx ?? 0;
      out.b = e.dy ?? 0;
      break;
    case "tap":
      out.x = e.x ?? 0;
      out.y = e.y ?? 0;
      out.a = e.count ?? 1;
      break;
    case "press":
      out.x = e.x ?? 0;
      out.y = e.y ?? 0;
      break;
    case "key": {
      const code = KEYS.indexOf(e.key ?? "");
      if (code < 0) throw new Error(`the key \`${String(e.key)}\` is not one the chart takes`);
      out.a = code;
      break;
    }
  }
  return out;
}

/**
 * The events of one animation frame (REQ-INPUT-030): `down`, `up`, `leave`,
 * `tap`, `press`, `key` kept in order; consecutive wheels summed (position
 * the latest); consecutive pinches multiplied and pans summed until one ends;
 * of the moves only the latest, sent after the rest.
 */
export class InputQueue {
  #list: Encoded[] = [];
  #move: Encoded | null = null;

  /** Whether anything waits. */
  get pending(): boolean {
    return this.#list.length > 0 || this.#move !== null;
  }

  push(e: Encoded): void {
    if (e.kind === InputKind.move) {
      this.#move = { ...e };
      return;
    }
    const last = this.#list[this.#list.length - 1];
    if (last && last.kind === e.kind && last.mods === e.mods) {
      if (e.kind === InputKind.wheel) {
        last.a += e.a;
        last.b += e.b;
        last.x = e.x;
        last.y = e.y;
        return;
      }
      if (e.kind === InputKind.pinch && last.b === 0) {
        last.a *= e.a;
        last.b = e.b;
        last.x = e.x;
        last.y = e.y;
        return;
      }
      if (e.kind === InputKind.pan && last.y === 0) {
        last.a += e.a;
        last.b += e.b;
        last.x = e.x;
        last.y = e.y;
        return;
      }
    }
    // A down, up or leave carries the pointer's latest position (or ends it),
    // so a move queued before it is superseded: at most one move a frame.
    if (e.kind === InputKind.up || e.kind === InputKind.down || e.kind === InputKind.leave) this.#move = null;
    this.#list.push({ ...e });
  }

  /** Everything queued, the latest move last, and empties the queue. */
  take(): Encoded[] {
    const out = this.#list;
    if (this.#move) out.push(this.#move);
    this.#list = [];
    this.#move = null;
    return out;
  }

  clear(): void {
    this.#list = [];
    this.#move = null;
  }
}

/** Gesture thresholds: 5 px slop and 500 ms between taps (interaction.md); the long press and double-tap distance are (unverified). */
export const GESTURE = { slopPx: 5, doubleTapMs: 500, doubleTapPx: 30, longPressMs: 500 } as const;

/** A pointer sample in CSS pixels relative to the drawing, and its cell position. */
export interface PointerSample {
  id: number;
  type: string;
  /** CSS px from the drawing's left and top. */
  px: number;
  py: number;
  /** Cells, with fractions. */
  x: number;
  y: number;
  buttons: number;
  mods: number;
  /** ms, as `event.timeStamp`. */
  time: number;
}

interface Down {
  startPx: number;
  startPy: number;
  startTime: number;
  last: PointerSample;
  moved: boolean;
}

/**
 * Turns pointer samples into boundary events. Mouse and pen pass through as
 * down, move and up, with a tap after a still click; touch is recognised
 * (one finger scrubs, two pan and pinch, tap and double tap, long press).
 * Timers come from the caller, so it runs without a DOM. The sequence is the
 * one api.ts describes.
 */
export class Gestures {
  readonly #emit: (e: Encoded) => void;
  readonly #setTimer: (f: () => void, ms: number) => unknown;
  readonly #clearTimer: (h: unknown) => void;
  #downs = new Map<number, Down>();
  #pressTimer: unknown = null;
  #pressed = false;
  #two: { cx: number; cy: number; spread: number; pinched: boolean; samples: { t: number; cx: number }[] } | null = null;
  #lastTap: { time: number; px: number; py: number } | null = null;

  constructor(emit: (e: Encoded) => void, setTimer: (f: () => void, ms: number) => unknown = setTimeout, clearTimer: (h: unknown) => void = (h) => clearTimeout(h as number)) {
    this.#emit = emit;
    this.#setTimer = setTimer;
    this.#clearTimer = clearTimer;
  }

  #send(kind: number, s: Pick<PointerSample, "x" | "y">, a = 0, b = 0, buttons = 0, mods = 0): void {
    this.#emit({ kind, x: s.x, y: s.y, a, b, buttons, mods });
  }

  #cancelPress(): void {
    if (this.#pressTimer !== null) this.#clearTimer(this.#pressTimer);
    this.#pressTimer = null;
  }

  #touches(): PointerSample[] {
    return [...this.#downs.values()].filter((d) => d.last.type === "touch").map((d) => d.last);
  }

  #startTwo(): void {
    const [a, b] = this.#touches();
    if (!a || !b) return;
    const cx = (a.x + b.x) / 2;
    const cy = (a.y + b.y) / 2;
    this.#two = { cx, cy, spread: Math.hypot(a.px - b.px, a.py - b.py) || 1, pinched: false, samples: [{ t: a.time, cx }] };
  }

  #tap(s: PointerSample): void {
    const last = this.#lastTap;
    const double = last !== null && s.time - last.time <= GESTURE.doubleTapMs && Math.hypot(s.px - last.px, s.py - last.py) <= GESTURE.doubleTapPx;
    this.#send(InputKind.tap, s, double ? 2 : 1);
    this.#lastTap = double ? null : { time: s.time, px: s.px, py: s.py };
  }

  down(s: PointerSample): void {
    this.#downs.set(s.id, { startPx: s.px, startPy: s.py, startTime: s.time, last: s, moved: false });
    if (s.type !== "touch") {
      this.#send(InputKind.down, s, 0, 0, buttonsOf(s.buttons, s.type), s.mods);
      return;
    }
    const touches = this.#touches();
    if (touches.length === 1) {
      this.#pressed = false;
      this.#send(InputKind.down, s, 0, 0, buttonsOf(1, "touch"), s.mods);
      this.#cancelPress();
      this.#pressTimer = this.#setTimer(() => {
        this.#pressTimer = null;
        const d = this.#downs.get(s.id);
        if (d && !d.moved && this.#touches().length === 1) {
          this.#pressed = true;
          this.#send(InputKind.press, d.last);
        }
      }, GESTURE.longPressMs);
    } else if (touches.length === 2) {
      this.#cancelPress();
      const first = touches.find((t) => t.id !== s.id);
      if (first && !this.#two) this.#send(InputKind.up, first, 0, 0, buttonsOf(0, "touch"), first.mods);
      this.#startTwo();
    }
  }

  move(s: PointerSample): void {
    const d = this.#downs.get(s.id);
    if (!d) {
      // Hover: a mouse or pen over the drawing with nothing pressed.
      if (s.type !== "touch") this.#send(InputKind.move, s, 0, 0, buttonsOf(s.buttons, s.type), s.mods);
      return;
    }
    d.last = s;
    if (!d.moved && Math.hypot(s.px - d.startPx, s.py - d.startPy) > GESTURE.slopPx) d.moved = true;
    if (s.type !== "touch") {
      this.#send(InputKind.move, s, 0, 0, buttonsOf(s.buttons, s.type), s.mods);
      return;
    }
    if (this.#two) {
      const [a, b] = this.#touches();
      if (!a || !b) return;
      const cx = (a.x + b.x) / 2;
      const cy = (a.y + b.y) / 2;
      const spread = Math.hypot(a.px - b.px, a.py - b.py) || 1;
      const dx = cx - this.#two.cx;
      const dy = cy - this.#two.cy;
      if (dx !== 0 || dy !== 0) this.#emit({ kind: InputKind.pan, x: 0, y: 0, a: dx, b: dy, buttons: 0, mods: 0 });
      const scale = spread / this.#two.spread;
      if (Math.abs(scale - 1) > 1e-3) {
        this.#emit({ kind: InputKind.pinch, x: cx, y: cy, a: scale, b: 0, buttons: 0, mods: 0 });
        this.#two.pinched = true;
        this.#two.spread = spread;
      }
      this.#two.cx = cx;
      this.#two.cy = cy;
      this.#two.samples.push({ t: s.time, cx });
      if (this.#two.samples.length > 8) this.#two.samples.shift();
      return;
    }
    if (d.moved) this.#cancelPress();
    this.#send(InputKind.move, s, 0, 0, buttonsOf(1, "touch"), s.mods);
  }

  /** A pointer lifted (`cancelled`: the browser took it, as for a vertical scroll). */
  up(s: PointerSample, cancelled = false): void {
    const d = this.#downs.get(s.id);
    if (!d) return;
    this.#downs.delete(s.id);
    if (s.type !== "touch") {
      this.#send(InputKind.up, s, 0, 0, buttonsOf(s.buttons, s.type), s.mods);
      if (!cancelled && !d.moved) this.#tap(s);
      return;
    }
    if (this.#two) {
      if (this.#touches().length < 2) this.#endTwo(s.time);
      return;
    }
    this.#cancelPress();
    this.#send(InputKind.up, s, 0, 0, buttonsOf(0, "touch"), s.mods);
    if (!cancelled && !d.moved && !this.#pressed && s.time - d.startTime < GESTURE.longPressMs) this.#tap(s);
    this.#pressed = false;
  }

  #endTwo(now: number): void {
    const two = this.#two;
    if (!two) return;
    this.#two = null;
    const first = two.samples[0];
    const last = two.samples[two.samples.length - 1];
    const dt = first && last ? (last.t - first.t) / 1000 : 0;
    const velocity = first && last && dt > 0 && now - last.t < 100 ? (last.cx - first.cx) / dt : 0;
    this.#emit({ kind: InputKind.pan, x: velocity, y: 1, a: 0, b: 0, buttons: 0, mods: 0 });
    if (two.pinched) this.#emit({ kind: InputKind.pinch, x: two.cx, y: two.cy, a: 1, b: 1, buttons: 0, mods: 0 });
  }

  /** Forgets every pointer and timer (stop). */
  reset(): void {
    this.#cancelPress();
    this.#downs.clear();
    this.#two = null;
    this.#lastTap = null;
    this.#pressed = false;
  }
}

// What examples' `live` functions share: a scope that undoes everything they
// started (timers, listeners, nodes, feeds) when the card leaves, and small
// builders for a control row, a status line and further charts. Nothing here
// touches the DOM until called, so Node can import the examples for their
// build-time stills (REQ-SITE-120).

import type { GlyphPlot } from "glyphplot/element";
import type { Stop } from "../api.ts";

/** A `<glyph-plot>` as examples use it. */
export type Plot = GlyphPlot;

/** Collects what a running example started, and stops it all at once. */
export class Scope {
  #stops: (() => void)[] = [];
  #stopped = false;

  /** Whether the scope has stopped: work that finishes later checks it before touching the page. */
  get stopped(): boolean {
    return this.#stopped;
  }

  /** Runs `f` when the scope stops (at once when it has). */
  add(f: () => void): void {
    if (this.#stopped) f();
    else this.#stops.push(f);
  }

  /** Listens for `type` on `target` until the scope stops. */
  on<E extends Event = CustomEvent>(target: EventTarget, type: string, f: (e: E) => void): void {
    const h = f as EventListener;
    target.addEventListener(type, h);
    this.add(() => target.removeEventListener(type, h));
  }

  /** Calls `f` every `ms` until the scope stops. */
  every(ms: number, f: () => void): void {
    const id = setInterval(f, ms);
    this.add(() => clearInterval(id));
  }

  /** Calls `f` once after `ms` unless the scope stops first. */
  after(ms: number, f: () => void): void {
    const id = setTimeout(f, ms);
    this.add(() => clearTimeout(id));
  }

  /** Appends `node` to `parent`, removed when the scope stops. */
  put<N extends Node>(parent: Node, node: N): N {
    parent.appendChild(node);
    this.add(() => node.parentNode?.removeChild(node));
    return node;
  }

  /** The function that stops everything, in reverse order. */
  get stop(): Stop {
    return () => {
      this.#stopped = true;
      for (const f of this.#stops.splice(0).reverse()) {
        try {
          f();
        } catch (e) {
          console.warn(`example: stopping: ${e instanceof Error ? e.message : String(e)}`);
        }
      }
    };
  }
}

/** An element of `tag` with `className` and text. */
export function make<K extends keyof HTMLElementTagNameMap>(tag: K, className = "", text = ""): HTMLElementTagNameMap[K] {
  const el = document.createElement(tag);
  if (className) el.className = className;
  if (text) el.textContent = text;
  return el;
}

/** A row of controls, labelled for assistive technology. */
export function controls(s: Scope, host: HTMLElement, label: string): HTMLDivElement {
  const row = make("div", "gp-ex-controls");
  row.setAttribute("role", "group");
  row.setAttribute("aria-label", label);
  return s.put(host, row);
}

/** A button in `row` calling `f`; with `pressed` it is a toggle whose state `f` returns. */
export function button(s: Scope, row: HTMLElement, text: string, f: () => boolean | void, pressed?: boolean): HTMLButtonElement {
  const b = make("button", "", text);
  b.type = "button";
  if (pressed !== undefined) b.setAttribute("aria-pressed", String(pressed));
  s.on(b, "click", () => {
    const on = f();
    if (pressed !== undefined && typeof on === "boolean") b.setAttribute("aria-pressed", String(on));
  });
  return s.put(row, b);
}

/** A Pause/Resume toggle for a streaming chart, kept in step with Space on the chart (REQ-A11Y-080). */
export function pauseButton(s: Scope, row: HTMLElement, el: Plot): HTMLButtonElement {
  const b = button(s, row, "Pause", () => {
    el.paused = !el.paused;
  });
  const show = (): void => {
    b.textContent = el.paused ? "Resume" : "Pause";
    b.setAttribute("aria-pressed", String(el.paused));
  };
  show();
  const mo = new MutationObserver(show);
  mo.observe(el, { attributes: true, attributeFilter: ["paused"] });
  s.add(() => mo.disconnect());
  return b;
}

/** A labelled `<select>` in `row` calling `f` with the chosen value. */
export function choice(s: Scope, row: HTMLElement, label: string, values: readonly string[], value: string, f: (v: string) => void): HTMLSelectElement {
  const l = make("label", "", `${label} `);
  const sel = make("select");
  for (const v of values) {
    const o = make("option", "", v);
    o.value = v;
    o.selected = v === value;
    sel.append(o);
  }
  s.on(sel, "change", () => f(sel.value));
  l.append(sel);
  s.put(row, l);
  return sel;
}

/** A polite status line under the chart. */
export function status(s: Scope, host: HTMLElement, text = ""): HTMLOutputElement {
  const o = make("output", "gp-ex-status", text);
  o.setAttribute("role", "status");
  return s.put(host, o);
}

/** A side-by-side row for several charts. */
export function grid(s: Scope, host: HTMLElement, columns = 2): HTMLDivElement {
  const g = make("div", "gp-ex-grid");
  g.style.setProperty("--gp-ex-columns", String(columns));
  return s.put(host, g);
}

/** One more `<glyph-plot>` in `parent`, with attributes and then properties set. */
export function plot(s: Scope, parent: HTMLElement, attrs: Record<string, string>, props: { data?: unknown; options?: Record<string, unknown>; view?: Record<string, unknown> } = {}): Plot {
  const el = document.createElement("glyph-plot") as Plot;
  for (const [k, v] of Object.entries(attrs)) el.setAttribute(k, v);
  if (props.options) el.options = props.options;
  if (props.data !== undefined) el.data = props.data as Plot["data"];
  if (props.view) el.view = props.view;
  return s.put(parent, el);
}

/** A caption above a chart in a grid cell. */
export function caption(s: Scope, parent: HTMLElement, text: string): HTMLElement {
  return s.put(parent, make("p", "gp-ex-caption", text));
}

/** A cell of a grid holding a caption and a chart. */
export function cell(s: Scope, g: HTMLElement, text: string, attrs: Record<string, string>, props: Parameters<typeof plot>[3] = {}): Plot {
  const box = s.put(g, make("div", "gp-ex-cell"));
  box.append(make("p", "gp-ex-caption", text));
  return plot(s, box, attrs, props);
}

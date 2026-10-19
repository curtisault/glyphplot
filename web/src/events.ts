// The reader-action events of one animation frame, rate-limited as
// REQ-EVENT-020 and REQ-EVENT-030 say, without the DOM: `plot-hover` only
// when the hovered datum changes and at most once a frame; `plot-view` with
// `final: false` at most once a frame, and exactly once with `final: true`
// when the gesture ends (pointer up, wheel idle 150 ms, key up, motion done),
// made by the shell when the module did not end it itself.

import type { WasmEvent } from "./api.ts";

/** How long a wheel must rest before its zoom is final (REQ-EVENT-030). */
export const WHEEL_IDLE_MS = 150;

/** What closes a gesture the module left open. */
export type GestureEnd = "pointer" | "wheel" | "key" | "motion" | "touch";

interface ViewDetail {
  source: string;
  cause: string;
  final: boolean;
  view: unknown;
}

function hoverKey(detail: unknown): string {
  if (detail === null || typeof detail !== "object") return "null";
  const d = detail as { series?: unknown; index?: unknown };
  return `${String(d.series)}\u0000${String(d.index)}`;
}

/**
 * Filters each frame's events. {@link EventGate.frame} takes the events the
 * module reported this frame and returns those to dispatch, in order.
 */
export class EventGate {
  #lastHover = "null";
  /** The open gesture: a `plot-view` with `final: false` dispatched and not yet closed. */
  #open: { source: string; cause: string } | null = null;

  /** Whether a view gesture is open. */
  get open(): { source: string; cause: string } | null {
    return this.#open;
  }

  frame(events: readonly WasmEvent[]): WasmEvent[] {
    const out: WasmEvent[] = [];
    let hover: WasmEvent | null = null;
    let view: WasmEvent | null = null;
    for (const e of events) {
      if (e.type === "plot-hover") {
        hover = e;
      } else if (e.type === "plot-view") {
        const d = e.detail as ViewDetail;
        if (d && d.final) {
          view = null; // the final supersedes this frame's earlier steps
          out.push(e);
          this.#open = null;
        } else {
          view = e;
        }
      } else {
        out.push(e);
      }
    }
    if (view) {
      const d = view.detail as ViewDetail;
      out.push(view);
      this.#open = { source: d.source, cause: d.cause };
    }
    if (hover) {
      const k = hoverKey(hover.detail);
      if (k !== this.#lastHover) {
        this.#lastHover = k;
        out.push(hover);
      }
    }
    return out;
  }

  /**
   * The final `plot-view` for the open gesture when `end` closes it, or null:
   * `pointer` and `touch` close a pointer or touch gesture, `wheel` a wheel
   * gesture, `key` a keyboard one, `motion` any.
   */
  close(end: GestureEnd, view: unknown): WasmEvent | null {
    const g = this.#open;
    if (!g) return null;
    const matches = end === "motion" || g.source === end || (end === "pointer" && g.source === "touch") || (end === "key" && g.source === "keyboard");
    if (!matches) return null;
    this.#open = null;
    return { type: "plot-view", detail: { source: g.source, cause: g.cause, final: true, view } };
  }

  /** Forgets the last hover (after a data replace a hover of the same row is news). */
  resetHover(): void {
    this.#lastHover = "null";
  }
}

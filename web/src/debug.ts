// The debug overlay (plan Phase 3, task 6): in a debug build only, a line in
// the corner of each chart giving the last display frame's cost against the
// chart's budget (`META.budget`, as web/pkg/budgets.json holds it). The shell
// draws it as a DOM element beside the drawing, outside the grid, so it never
// changes a frame's cells, colours or backs. The element imports this module
// on demand when the manifest is a debug build's (`fallback` not null), so a
// release page never fetches it.

import type { Meta } from "./api.ts";
import type { FrameTiming } from "./mount.ts";

/** A chart's budget, in ms. */
export type Budget = Pick<Meta["budget"], "frameMs" | "worstMs" | "inputMs">;

/** The overlay on one chart. */
export interface Overlay {
  /** Shows a frame's cost; frames that drew nothing are ignored. */
  update(t: Readonly<FrameTiming>): void;
  /** Takes the overlay off. */
  remove(): void;
}

/** Frames the overlay keeps for its worst figure. */
const WINDOW = 60;

const ms = (v: number): string => (v < 10 ? v.toFixed(2) : v.toFixed(1));

/**
 * Puts the overlay in `host`'s bottom-right corner: `wasm` is the chart's
 * frame (`chart.frame`), `draw` the canvas draw, `input` the inputs applied
 * that frame, and `worst` the slowest wasm + draw of the last 60 frames. It
 * turns amber when the wasm frame passes `frameMs` and red when a frame
 * passes `worstMs` or the input passes `inputMs`.
 */
export function debugOverlay(host: HTMLElement, budget: Budget): Overlay {
  const doc = host.ownerDocument;
  const el = doc.createElement("div");
  el.className = "gp-debug";
  el.setAttribute("aria-hidden", "true");
  el.style.cssText =
    "position:absolute;right:0;bottom:0;z-index:2;pointer-events:none;padding:1px 4px;font:11px/1.3 ui-monospace,monospace;white-space:pre;color:#fff;background:rgba(0,0,0,.72);border-radius:3px 0 0 0";
  host.append(el);
  const recent: number[] = [];
  return {
    update(t) {
      if (!t.drew) return;
      const total = t.frame + t.draw;
      recent.push(total);
      if (recent.length > WINDOW) recent.shift();
      const worst = Math.max(...recent);
      const over = worst > budget.worstMs || t.input > budget.inputMs;
      const warn = t.frame > budget.frameMs;
      el.style.background = over ? "rgba(170,20,20,.85)" : warn ? "rgba(150,100,0,.85)" : "rgba(0,0,0,.72)";
      const input = t.input > 0 ? ` · input ${ms(t.input)}/${budget.inputMs}` : "";
      el.textContent = `wasm ${ms(t.frame)}/${budget.frameMs} · draw ${ms(t.draw)}${input} · worst ${ms(worst)}/${budget.worstMs} ms`;
    },
    remove() {
      el.remove();
    },
  };
}

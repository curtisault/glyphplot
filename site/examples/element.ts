// STUB, written by the site agent while site/examples/element.ts did not
// exist in its worktree; the examples agent's file replaces it. It defines
// <gp-example slug="…"> with the least the site needs: the example's chart
// from `start()` (no build-time still yet) and its `live` once drawn.
import "glyphplot/element";
import type { GlyphPlot } from "glyphplot/element";
import { exampleOf } from "./index.ts";
import type { Stop } from "./api.ts";

class GpExample extends HTMLElement {
  #stop: Stop | null = null;

  connectedCallback(): void {
    const slug = this.getAttribute("slug") ?? "";
    const example = exampleOf(slug);
    if (!example) {
      this.textContent = `No example named "${slug}".`;
      return;
    }
    const start = example.start();
    const el = document.createElement("glyph-plot") as GlyphPlot;
    el.setAttribute("type", start.type);
    el.setAttribute("label", slug);
    (el as unknown as { data: unknown }).data = start.data;
    if (start.options) el.options = start.options;
    if (start.view) el.view = start.view;
    this.replaceChildren(el);
    el.ready.then(
      () => {
        if (el.isConnected && example.live) this.#stop = example.live(el, this);
      },
      () => {},
    );
  }

  disconnectedCallback(): void {
    this.#stop?.();
    this.#stop = null;
  }
}

if (!customElements.get("gp-example")) customElements.define("gp-example", GpExample);

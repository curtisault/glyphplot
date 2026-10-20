// The load bench's page (REQ-PERF-080, REQ-BENCH-020): one <glyph-plot> line
// chart of 1,000 points, as discovery's load page drew (discovery/budgets.md,
// "Load"). On `plot-ready`, which the element fires right after its first
// frame is drawn, it sets the `glyphplot:first-frame` mark and
// `window.firstFrame`, ms from navigation start (performance.now()), with
// the resource timings; bench/load.mjs reads them.
import "glyphplot/element";

interface Resource {
  name: string;
  transfer: number;
  end: number;
}

declare global {
  interface Window {
    firstFrame?: { ms: number; resources: Resource[] };
  }
}

const n = 1000;
const x = new Float64Array(n);
const y = new Float64Array(n);
for (let i = 0; i < n; i++) {
  x[i] = i;
  y[i] = Math.sin(i / 50);
}

const el = document.getElementById("chart") as (HTMLElement & { data: unknown }) | null;
if (!el) throw new Error("load.html: no #chart");
el.addEventListener(
  "plot-ready",
  () => {
    performance.mark("glyphplot:first-frame");
    const ms = performance.now();
    const resources = performance.getEntriesByType("resource").map((e) => {
      const r = e as PerformanceResourceTiming;
      return { name: r.name.split("/").slice(-1)[0] ?? r.name, transfer: r.transferSize, end: r.responseEnd };
    });
    window.firstFrame = { ms, resources };
  },
  { once: true },
);
el.data = { x, y };

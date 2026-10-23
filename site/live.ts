// live.html: the owner's one-line test for v0.6.0 (the plan's Phase 5
// acceptance): a live candlestick on the seeded synthetic feed, labelled
// "simulated". It imports the element alone, so the page plays one wasm
// module (candles). The view follows the live end (x.follow, REQ-VIEW-060);
// a pan away stops it, End or the button starts it again; Space on the chart
// or the button pauses (REQ-A11Y-080).
import "glyphplot/element";
import type { GlyphPlot } from "glyphplot/element";
import { candleColumns, Feed } from "./feed.ts";

const el = document.getElementById("live") as GlyphPlot;
const status = document.getElementById("status") as HTMLOutputElement;
const pause = document.getElementById("pause") as HTMLButtonElement;
const feed = new Feed(2026, 5000);
const start = Date.now();
const history = feed.history(240, start);
el.data = candleColumns(history);
const span = 60 * feed.period;
const last = history[history.length - 1]?.t ?? start;
el.view = { x: { domain: [last - span, last], follow: true } };

let commits = 0;
let late = 0;
el.addEventListener("plot-data", (e) => {
  const d = (e as CustomEvent<{ rows: number; dropped: number; late: number; version: number }>).detail;
  commits++;
  late += d.late;
  const x = el.view["x"] as { follow?: boolean } | undefined;
  status.textContent = `simulated feed: ${d.rows} candles held, data version ${d.version}, ${commits} commits, ${late} late rows; ${el.paused ? "paused" : "live"}; follow ${x?.follow ? "on" : "off"}; module ${el.chart?.streaming === "staging" ? "staging(n) + append(n)" : "re-commit fallback"}`;
});
el.addEventListener("plot-error", (e) => (status.textContent = `error: ${(e as CustomEvent<{ message: string }>).detail.message}`));

setInterval(() => {
  const c = feed.tick(Date.now());
  el.append({ t: new Date(c.t), open: c.open, high: c.high, low: c.low, close: c.close, volume: c.volume });
}, 250);

const showPause = (): void => {
  pause.textContent = el.paused ? "Resume" : "Pause";
  pause.setAttribute("aria-pressed", String(el.paused));
};
pause.addEventListener("click", () => {
  el.paused = !el.paused;
  showPause();
});
new MutationObserver(showPause).observe(el, { attributes: true, attributeFilter: ["paused"] });
document.getElementById("follow")?.addEventListener("click", () => (el.view = { x: { follow: true } }));

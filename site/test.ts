// site/test.html: the owner's checklist (REQ-TEST-040). One <glyph-plot> (or
// one mount()) per rendering and interaction case, its data set from here, a
// mark and a note per case kept in this browser, and a button that copies the
// pass as Markdown. Plain TypeScript over the shell's own modules: no Elm.

import * as gp from "glyphplot";
import { config, define, type GlyphPlot } from "glyphplot/element";
import { Feed } from "./feed.ts";

declare global {
  interface Window {
    gp: typeof gp;
    gpReady: true;
  }
}
window.gp = gp;

const $ = <T extends Element = HTMLElement>(sel: string): T => {
  const el = document.querySelector<T>(sel);
  if (!el) throw new Error(`test.html: no ${sel}`);
  return el;
};
const plot = (id: string): GlyphPlot => $<GlyphPlot>(`#${id}`);

// ------------------------------------------------------------ data

/** A deterministic generator (mulberry32), so every pass draws the same data. */
function rng(seed: number): () => number {
  let a = seed >>> 0;
  return () => {
    a = (a + 0x6d2b79f5) >>> 0;
    let t = a;
    t = Math.imul(t ^ (t >>> 15), t | 1);
    t ^= t + Math.imul(t ^ (t >>> 7), t | 61);
    return ((t ^ (t >>> 14)) >>> 0) / 4294967296;
  };
}

/** n points of `names.length` waves over x = 0 … n−1. */
function waves(n: number, names: string[], seed = 1): Record<string, number[]> {
  const r = rng(seed);
  const out: Record<string, number[]> = { x: [] };
  names.forEach((s) => (out[s] = []));
  for (let i = 0; i < n; i++) {
    out["x"]?.push(i);
    names.forEach((s, k) => out[s]?.push(Math.sin(i / (9 + 4 * k) + k) * (10 + 3 * k) + (r() - 0.5) * 2 + 5 * k));
  }
  return out;
}

const DAY = 86_400_000;
const START = Date.UTC(2025, 0, 1);

/** Daily values of two series from 2025-01-01, as rows with Date x (a timestamp column). */
function daily(n: number, seed: number, a = "north", b = "south"): Record<string, (Date | number)[]> {
  const r = rng(seed);
  const t: Date[] = [];
  const s1: number[] = [];
  const s2: number[] = [];
  let w1 = 12;
  let w2 = 18;
  for (let i = 0; i < n; i++) {
    t.push(new Date(START + i * DAY));
    w1 += (r() - 0.5) * 2;
    w2 += (r() - 0.5) * 2;
    s1.push(Math.round((w1 + 8 * Math.sin((2 * Math.PI * i) / 365)) * 10) / 10);
    s2.push(Math.round((w2 + 6 * Math.sin((2 * Math.PI * i) / 365 + 1)) * 10) / 10);
  }
  return { t, [a]: s1, [b]: s2 };
}

/** SplitMix64, as `crates/glyphplot/src/fixtures.rs` has it: the same stream, so the page draws the fixtures' own data. */
function splitmix(seed: bigint): { f64: () => number; below: (n: number) => number } {
  const M = (1n << 64n) - 1n;
  let s = seed & M;
  const next = (): bigint => {
    s = (s + 0x9e3779b97f4a7c15n) & M;
    let z = s;
    z = ((z ^ (z >> 30n)) * 0xbf58476d1ce4e5b9n) & M;
    z = ((z ^ (z >> 27n)) * 0x94d049bb133111ebn) & M;
    return z ^ (z >> 31n);
  };
  return { f64: () => Number(next() >> 11n) / 9_007_199_254_740_992, below: (n) => Number(next() % BigInt(n)) };
}

/** The `ohlcv-days` fixture's rows: limit down at row 250, no trades at row 333. */
const LIMIT_DOWN_ROW = 250;
const ZERO_VOLUME_ROW = 333;

/** The `ohlcv-days` fixture (fixtures.rs `ohlcv_days`): 500 weekday bars from Monday 2024-10-21, generated as the core generates it. */
function ohlcvDays(n = 500): { t: Date[]; open: number[]; high: number[]; low: number[]; close: number[]; volume: number[] } {
  // The fixture's seed: "glyplot" and its number, 19.
  const r = splitmix(0x676c_7970_6c6f_7400n + 19n);
  const out = { t: [] as Date[], open: [] as number[], high: [] as number[], low: [] as number[], close: [] as number[], volume: [] as number[] };
  let day = 20_017;
  let last = 100;
  for (let i = 0; i < n; i++) {
    while ((day + 4) % 7 === 0 || (day + 4) % 7 === 6) day++;
    out.t.push(new Date(day * DAY));
    day++;
    const open = last * (1 + 0.01 * (r.f64() - 0.5));
    let bar: [number, number, number, number];
    let volume: number;
    if (i === LIMIT_DOWN_ROW) {
      const close = open * 0.9;
      bar = [open, open, close, close];
      volume = 5_000_000;
    } else if (i === ZERO_VOLUME_ROW) {
      bar = [last, last, last, last];
      volume = 0;
    } else {
      const close = open * (1 + 0.04 * (r.f64() - 0.5));
      const high = (open > close ? open : close) * (1 + 0.01 * r.f64());
      const low = (open < close ? open : close) * (1 - 0.01 * r.f64());
      bar = [open, high, low, close];
      volume = 100_000 + r.below(900_000);
    }
    out.open.push(bar[0]);
    out.high.push(bar[1]);
    out.low.push(bar[2]);
    out.close.push(bar[3]);
    out.volume.push(volume);
    last = bar[3];
  }
  return out;
}

// ------------------------------------------------------------ the environment and the pass

const STORE = "glyphplot-checklist-v0.6.0";
type Mark = { state: "" | "pass" | "fail" | "na"; note: string };
const marks: Record<string, Mark> = (() => {
  try {
    return JSON.parse(localStorage.getItem(STORE) ?? "{}") as Record<string, Mark>;
  } catch {
    return {};
  }
})();
const save = (): void => {
  try {
    localStorage.setItem(STORE, JSON.stringify(marks));
  } catch {
    // private mode: marks last as long as the page
  }
};

let build = "unknown";
const env = (): [string, string][] => [
  ["Browser", navigator.userAgent],
  ["Pixel ratio", String(devicePixelRatio)],
  ["Viewport", `${innerWidth} × ${innerHeight} CSS px`],
  ["Scheme", `${matchMedia("(prefers-color-scheme: dark)").matches ? "dark" : "light"}${document.documentElement.dataset["theme"] ? ` (page: ${document.documentElement.dataset["theme"]})` : ""}`],
  ["Reduced motion", matchMedia("(prefers-reduced-motion: reduce)").matches ? "on" : "off"],
  ["Pointer", matchMedia("(pointer: coarse)").matches ? "coarse (touch)" : "fine"],
  ["Build", build],
  ["Renderer", config.renderer],
  ["Page", location.href],
];

function showEnv(): void {
  const dl = $("#env");
  dl.replaceChildren();
  for (const [k, v] of env()) {
    const dt = document.createElement("dt");
    dt.textContent = k;
    const dd = document.createElement("dd");
    dd.textContent = v;
    dl.append(dt, dd);
  }
}

function addMarks(): void {
  for (const sec of document.querySelectorAll<HTMLElement>("section.case")) {
    const id = sec.id;
    const title = sec.dataset["title"] ?? id;
    const m = (marks[id] ??= { state: "", note: "" });
    const box = document.createElement("div");
    box.className = "result";
    box.setAttribute("role", "group");
    box.setAttribute("aria-label", `Result: ${title}`);
    box.dataset["state"] = m.state;
    for (const [value, text] of [["pass", "Pass"], ["fail", "Fail"], ["na", "Not run"]] as const) {
      const label = document.createElement("label");
      const input = document.createElement("input");
      input.type = "radio";
      input.name = `mark-${id}`;
      input.value = value;
      input.checked = m.state === value;
      input.addEventListener("change", () => {
        m.state = value;
        box.dataset["state"] = value;
        save();
      });
      label.append(input, text);
      box.append(label);
    }
    const note = document.createElement("input");
    note.type = "text";
    note.placeholder = "note (what failed, where)";
    note.setAttribute("aria-label", `Note: ${title}`);
    note.value = m.note;
    note.addEventListener("input", () => {
      m.note = note.value;
      save();
    });
    box.append(note);
    sec.append(box);
  }
}

/** The pass as Markdown (REQ-TEST-040). */
export function markdown(): string {
  const cell = (s: string): string => s.replace(/\|/g, "\\|").replace(/\n/g, " ");
  const lines = [`### glyphplot v0.6.0 checklist, ${new Date().toISOString().slice(0, 10)}`, ""];
  for (const [k, v] of env()) lines.push(`- ${k}: ${cell(v)}`);
  lines.push("", "| # | Case | Result | Note |", "|---|---|---|---|");
  let i = 0;
  for (const sec of document.querySelectorAll<HTMLElement>("section.case")) {
    i++;
    const m = marks[sec.id] ?? { state: "", note: "" };
    const result = m.state === "pass" ? "pass" : m.state === "fail" ? "**FAIL**" : m.state === "na" ? "not run" : "—";
    lines.push(`| ${i} | ${cell(sec.dataset["title"] ?? sec.id)} | ${result} | ${cell(m.note)} |`);
  }
  return lines.join("\n");
}

async function copy(): Promise<void> {
  const text = markdown();
  const area = $<HTMLTextAreaElement>("#markdown");
  area.value = text;
  area.hidden = false;
  const status = $("#copied");
  try {
    if (!navigator.clipboard) throw new Error("no clipboard API (not a secure context)");
    await navigator.clipboard.writeText(text);
    status.textContent = "Copied.";
  } catch {
    // http on a phone over the LAN has no clipboard API: select the text and try the old way.
    area.focus();
    area.select();
    const ok = document.execCommand?.("copy") ?? false;
    status.textContent = ok ? "Copied." : "Select the text below and copy it.";
  }
}

// ------------------------------------------------------------ logs

function logger(box: HTMLElement): (line: string) => void {
  return (line) => {
    const now = new Date();
    const stamp = `${now.toTimeString().slice(0, 8)}.${String(now.getMilliseconds()).padStart(3, "0")}`;
    box.textContent = `${stamp} ${line}\n${box.textContent ?? ""}`.split("\n").slice(0, 200).join("\n");
  };
}

const EVENTS = ["plot-ready", "plot-error", "plot-hover", "plot-select", "plot-view", "plot-cursor", "plot-brush", "plot-legend"] as const;

function short(type: string, d: unknown): string {
  if (d === null) return `${type} null`;
  const o = d as Record<string, unknown>;
  const num = (v: unknown): string => (typeof v === "number" ? String(Math.round(v * 1000) / 1000) : JSON.stringify(v));
  switch (type) {
    case "plot-view": {
      const v = o["view"] as { x?: { domain?: unknown } } | undefined;
      const dom = v?.x?.domain;
      return `${type} ${String(o["source"])} ${String(o["cause"])} final=${String(o["final"])} x=${Array.isArray(dom) ? dom.map(num).join("…") : "fit"}`;
    }
    case "plot-hover":
      return `${type} ${String(o["source"])} ${String(o["series"])}[${String(o["index"])}] x=${num(o["x"])} y=${num(o["y"])}`;
    case "plot-brush":
      return `${type} ${String(o["source"])} x=${Array.isArray(o["x"]) ? (o["x"] as unknown[]).map(num).join("…") : "—"} count=${String(o["count"])} final=${String(o["final"])}`;
    case "plot-legend":
      return `${type} ${String(o["source"])} ${String(o["series"])} visible=${String(o["visible"])} hidden=${JSON.stringify(o["hidden"])}`;
    default:
      return `${type} ${JSON.stringify(d).slice(0, 160)}`;
  }
}

function logEvents(el: HTMLElement, box: HTMLElement): (line: string) => void {
  const log = logger(box);
  for (const t of EVENTS) el.addEventListener(t, (e) => log(short(t, (e as CustomEvent).detail)));
  return log;
}

// ------------------------------------------------------------ the cases

/** The theme object a JS-API mount gets on this page: the light set on a light page. */
function pageTheme(): gp.Theme {
  const dark = document.documentElement.dataset["theme"] === "dark" || (document.documentElement.dataset["theme"] !== "light" && matchMedia("(prefers-color-scheme: dark)").matches);
  return dark ? {} : { paper: true };
}

/** Cases 1 and 2: the JS API's mount() on a <pre> and on canvases with each renderer. */
async function mounts(): Promise<void> {
  const data = waves(160, ["a", "b"], 3);
  const dpr = devicePixelRatio || 1;
  const cw = Math.round(0.6 * 13 * dpr);
  const cols = 40;
  const rows = 10;
  for (const el of document.querySelectorAll<HTMLCanvasElement | HTMLPreElement>("[data-mount]")) {
    const kind = el.dataset["mount"] ?? "canvas";
    const c = gp.chart("line", { data });
    await c.ready;
    const palette = c.palette(pageTheme());
    const stats = document.querySelector<HTMLOutputElement>(`[data-stats="${kind}"]`);
    const m = gp.mount(el, c, {
      input: true,
      renderer: kind === "atlas" ? "atlas" : "text",
      palette,
      paintGround: false,
      waitForSize: true,
      onFrame: (_g, s) => {
        if (stats && s) stats.textContent = `last frame: ${s.cells} of ${cols * rows} cells redrawn${s.full ? " (full)" : ""}`;
      },
    });
    if (el instanceof HTMLCanvasElement) Object.assign(el.style, { width: `${(cols * cw) / dpr}px`, height: `${(rows * cw * 2) / dpr}px` });
    else Object.assign(el.style, { fontSize: `${cw / 0.6 / dpr}px`, lineHeight: `${(cw * 2) / dpr}px` });
    m.resize(cols, rows, cw);
  }
}

function modes(): void {
  for (const id of ["mode-line", "mode-step", "mode-area"]) plot(id).data = waves(48, ["up", "down"], 7);
}

function gaps(): void {
  const d = waves(120, ["y"], 11);
  const y = (d["y"] ?? []).map((v, i) => ((i >= 30 && i < 38) || (i >= 75 && i < 80) ? null : v));
  for (const id of ["gaps-off", "gaps-on"]) plot(id).data = { x: d["x"] ?? [], y };
}

function million(): void {
  const el = plot("million-chart");
  const status = $("#million-status");
  const n = 1_000_000;
  const x = new Float64Array(n);
  const y = new Float64Array(n);
  const r = rng(42);
  let w = 0;
  for (let i = 0; i < n; i++) {
    x[i] = i;
    w += r() - 0.5;
    y[i] = w + (i % 99_991 === 0 ? 40 : 0); // a spike every ~100k points: M4 must keep them
  }
  el.data = { x, y };
  let near = 0;
  const io = new IntersectionObserver(
    (entries) => {
      if (entries.some((e) => e.isIntersecting) && near === 0) {
        near = performance.now();
        io.disconnect();
      }
    },
    { rootMargin: "100% 0px" },
  );
  io.observe(el);
  el.addEventListener("plot-ready", () => {
    status.textContent = `first frame ${Math.round(performance.now() - near)} ms after the chart came within a screen (performance.now(), this page load)`;
  });
}

function mono(): void {
  plot("mono-chart").data = waves(150, ["solid", "dashed", "dotted"], 5);
}

function lightDark(): void {
  for (const id of ["theme-light", "theme-dark"]) plot(id).data = waves(100, ["a", "b", "c"], 9);
}

function themeSwitch(): void {
  const el = plot("theme-chart");
  el.data = waves(100, ["a", "b", "c", "d"], 13);
  const status = $("#theme-status");
  let frames = 0;
  const show = (): void => {
    status.textContent = `frames drawn: ${frames}; data-theme: ${document.documentElement.dataset["theme"] || "(system)"}`;
  };
  void el.ready.then(() => {
    const c = el.chart;
    if (!c) return;
    const frame = c.frame.bind(c);
    c.frame = (t?: number, paper?: boolean) => {
      frames++;
      show();
      return frame(t, paper);
    };
    show();
  });
  for (const b of document.querySelectorAll<HTMLButtonElement>("[data-page-theme]")) {
    b.addEventListener("click", () => {
      const v = b.dataset["pageTheme"];
      if (v) document.documentElement.dataset["theme"] = v;
      else delete document.documentElement.dataset["theme"];
      showEnv();
      show();
    });
  }
  $("#theme-prop").addEventListener("click", () => el.style.setProperty("--glyphplot-series-1", "#d81b60"));
  $("#theme-object").addEventListener("click", () => {
    el.theme = { series: ["#e66100", "#5d3a9b", "#1a85ff", "#40b0a6"] };
  });
  $("#theme-clear").addEventListener("click", () => {
    el.style.removeProperty("--glyphplot-series-1");
    el.theme = null;
  });
}

function wideLabels(): void {
  const el = plot("wide-chart");
  el.options = { title: "売上高 (百万円) – Umsatz je Quartal" };
  const d = waves(80, ["Umsatz", "Kosten"], 17);
  for (const k of ["Umsatz", "Kosten"]) d[k] = (d[k] ?? []).map((v) => 1_234_500_000 + v * 25_000_000);
  el.data = d;
}

function resize(): void {
  const el = plot("resize-chart");
  const box = $("#resize-box");
  const out = $("#resize-status");
  el.data = waves(200, ["a", "b"], 19);
  const show = (): void => {
    const dom = (el.view["x"] as { domain?: unknown } | undefined)?.domain;
    out.textContent = `${el.cols} × ${el.rows} cells, box ${Math.round(box.getBoundingClientRect().width)} px; x domain ${Array.isArray(dom) ? dom.map((v: number) => v.toFixed(2)).join(" … ") : "fitted"}`;
  };
  new ResizeObserver(() => requestAnimationFrame(show)).observe(box);
  el.addEventListener("plot-ready", show);
  el.addEventListener("plot-view", show);
  for (const b of document.querySelectorAll<HTMLButtonElement>("[data-width]")) {
    b.addEventListener("click", () => {
      box.style.width = b.dataset["width"] ?? "";
    });
  }
}

function script(): void {
  const el = plot("script-chart");
  const out = $("#script-view-out");
  const log = logEvents(el, $("#script-log"));
  const show = (what: string): void => {
    requestAnimationFrame(() => {
      out.textContent = `after ${what}: view.x = ${JSON.stringify(el.view["x"])}`;
    });
  };
  el.data = waves(60, ["a"], 23);
  let seed = 23;
  $("#script-data").addEventListener("click", () => {
    seed++;
    el.data = waves(60, ["a"], seed);
    log("(script) data set");
    show("data");
  });
  $("#script-options").addEventListener("click", () => {
    el.options = { mode: el.options["mode"] === "area" ? "line" : "area" };
    log(`(script) options set: mode ${String(el.options["mode"])}`);
    show("options");
  });
  $("#script-view").addEventListener("click", () => {
    el.view = { x: { domain: [20, 40] } };
    log("(script) view set: x 20…40");
    show("view");
  });
  $("#script-reset").addEventListener("click", () => {
    el.resetView();
    log("(script) view reset");
    show("reset");
  });
}

function interaction(): void {
  const p = plot("pointer-chart");
  p.data = daily(400, 29);
  logEvents(p, $("#pointer-log"));
  for (const id of ["keys-chart", "keys-motion"]) plot(id).data = daily(120, 31);
  const t = plot("touch-chart");
  t.data = daily(200, 37);
  logEvents(t, $("#touch-log"));
  plot("sr-chart").data = daily(1000, 41, "Oslo", "Lisbon");
}

/** Case 16: the debug build's `debug-panic` chart beside a healthy one, and the shadow stack around the trap. */
async function panic(): Promise<void> {
  const status = $("#panic-status");
  const m = await gp.manifest();
  if (m.fallback === null) {
    build = "release";
    status.textContent = "This is the release build: the panic case runs under `mise run dev` (the debug build).";
    showEnv();
    return;
  }
  build = `debug (module ${m.fallback})`;
  showEnv();
  const loaded = await gp.loadModule(m.fallback);
  const before = loaded.stack ? loaded.stack(0) : null;
  const box = $("#panic-box");
  const bad = document.createElement("glyph-plot") as GlyphPlot;
  bad.setAttribute("type", "debug-panic");
  bad.setAttribute("label", "Panics when framed");
  bad.setAttribute("loading", "eager");
  bad.textContent = "This text stays: the chart failed.";
  bad.data = { x: [0, 1], y: [0, 1] };
  const good = document.createElement("glyph-plot") as GlyphPlot;
  good.setAttribute("type", "line");
  good.setAttribute("label", "Keeps drawing");
  good.setAttribute("loading", "eager");
  good.data = waves(80, ["a"], 43);
  box.append(bad, good);
  bad.addEventListener("plot-error", (e) => {
    const after = loaded.stack ? loaded.stack(0) : null;
    status.textContent = `plot-error: ${String((e as CustomEvent<{ message: string }>).detail.message)}\nshadow stack pointer before ${String(before)}, after ${String(after)}: ${before === after ? "the same" : "MOVED"}`;
  });
}

function lazy(): void {
  const el = plot("lazy-chart");
  el.data = waves(90, ["a", "b"], 47);
  el.addEventListener("plot-ready", () => {
    $("#lazy-status").textContent = `loaded and drawn at scrollY ${Math.round(scrollY)} px (the chart's top is at ${Math.round(el.getBoundingClientRect().top + scrollY)} px)`;
  });
}

/** Case 18: a tag made by define(), its type fixed whatever its attribute says. */
function alias(): void {
  define("line-plot", { type: "line" });
  const el = $<GlyphPlot>("#alias-chart");
  el.data = waves(80, ["a", "b"], 53);
  const show = (): void => {
    $("#alias-status").textContent = `type attribute "${el.getAttribute("type") ?? ""}", chart type "${el.type}", drawn: ${el.chart ? "yes" : "no"}`;
  };
  el.addEventListener("plot-ready", show);
  $("#alias-type").addEventListener("click", () => {
    el.setAttribute("type", "bar");
    requestAnimationFrame(show);
  });
}

/** Case 19: a type change carrying the data and view, or warning and drawing empty. */
function typeChange(): void {
  const el = plot("type-chart");
  const out = $("#type-status");
  el.data = waves(120, ["a", "b"], 59);
  const show = (what: string): void => {
    const d = el.data;
    const rows = d ? Object.values(d.tables)[0]?.length : null;
    out.textContent = `${what}: type ${el.type}; data rows ${rows ?? "none"}; x domain ${JSON.stringify((el.view["x"] as { domain?: unknown } | undefined)?.domain ?? null)}`;
  };
  el.addEventListener("plot-ready", () => show("ready"));
  el.addEventListener("plot-error", (e) => show(`plot-error (${(e as CustomEvent<{ message: string }>).detail.message})`));
  for (const b of document.querySelectorAll<HTMLButtonElement>("[data-type]")) {
    b.addEventListener("click", () => el.setAttribute("type", b.dataset["type"] ?? "line"));
  }
}

/** Case 20: fit width and contain. */
function fit(): void {
  const w = plot("fit-width");
  const c = plot("fit-contain");
  const out = $("#fit-status");
  for (const el of [w, c]) el.data = waves(100, ["a", "b"], 61);
  const show = (): void => {
    out.textContent = [w, c]
      .map((el) => {
        const f = el.querySelector<HTMLElement>(".gp-frame")?.getBoundingClientRect();
        return `${el.id}: fit ${el.getAttribute("fit") ?? "none"}, ${el.cols} × ${el.rows} cells, drawn ${Math.round(f?.width ?? 0)} × ${Math.round(f?.height ?? 0)} px`;
      })
      .join("; ");
  };
  for (const el of [w, c]) new ResizeObserver(() => requestAnimationFrame(show)).observe(el);
  for (const b of document.querySelectorAll<HTMLButtonElement>("[data-fit]")) {
    b.addEventListener("click", () => {
      const on = b.dataset["fit"] !== "";
      w.toggleAttribute("fit", on);
      c.toggleAttribute("fit", on);
      if (on) {
        w.setAttribute("fit", "width");
        c.setAttribute("fit", "contain");
      }
      requestAnimationFrame(show);
    });
  }
  for (const b of document.querySelectorAll<HTMLButtonElement>("[data-fit-width]")) {
    b.addEventListener("click", () => {
      for (const box of document.querySelectorAll<HTMLElement>(".fit-box")) box.style.width = b.dataset["fitWidth"] ?? "";
    });
  }
}

/** Case 21: two linked charts of different widths. */
function link(): void {
  const log = logger($("#link-log"));
  for (const id of ["link-a", "link-b"]) {
    const el = plot(id);
    el.data = daily(300, 67);
    for (const t of ["plot-view", "plot-hover", "plot-cursor"]) el.addEventListener(t, (e) => log(`${id}: ${short(t, (e as CustomEvent).detail)}`));
  }
}

/** Case 22: brush and legend events. */
function brushLegend(): void {
  const el = plot("events-chart");
  el.data = daily(200, 71, "inside", "outside");
  const log = logger($("#events-log"));
  for (const t of ["plot-brush", "plot-legend"]) el.addEventListener(t, (e) => log(short(t, (e as CustomEvent).detail)));
}

/** Case 23: the timeZone attribute and the table the core was given. */
function timeZone(): void {
  const el = plot("zone-chart");
  const out = $("#zone-status");
  const t: Date[] = [];
  const y: number[] = [];
  for (let i = 0; i < 72; i++) {
    t.push(new Date(Date.UTC(2026, 2, 7) + i * 3_600_000));
    y.push(Math.round(Math.sin(i / 6) * 100) / 10);
  }
  el.data = { t, y };
  const show = (): void => {
    const z = el.chart?.zone;
    if (!z) return;
    const changes = z.transitions.map((x) => `${new Date(x.at).toISOString().slice(0, 16)}Z ${x.offset >= 0 ? "+" : "−"}${Math.abs(x.offset)} min`).join(", ");
    out.textContent = `page zone ${gp.pageZone()}; table "${z.name}": ${changes}; covering ${new Date(z.covering[0]).toISOString().slice(0, 10)} to ${new Date(z.covering[1]).toISOString().slice(0, 10)}`;
  };
  el.addEventListener("plot-ready", show);
  el.addEventListener("plot-view", () => requestAnimationFrame(show));
  $<HTMLSelectElement>("#zone-pick").addEventListener("change", (e) => {
    const v = (e.target as HTMLSelectElement).value;
    if (v) el.setAttribute("timeZone", v);
    else el.removeAttribute("timeZone");
    requestAnimationFrame(show);
  });
}

/** Case 24: a heatmap-like frame of back colours through both renderers. */
async function backs(): Promise<void> {
  const c = gp.chart("line");
  await c.ready;
  const palette = c.palette(pageTheme());
  c.free();
  const cols = 48;
  const rows = 12;
  const n = cols * rows;
  const dpr = devicePixelRatio || 1;
  const cw = Math.round(0.6 * 13 * dpr);
  const grid = { cols, rows, cells: new Uint32Array(n).fill(32), colors: new Uint8Array(n), backs: new Uint8Array(n) };
  let shift = 0;
  const fill = (): void => {
    for (let r = 0; r < rows; r++) {
      for (let k = 0; k < cols; k++) {
        const i = r * cols + k;
        grid.backs[i] = 32 + Math.round(63.5 + 63.5 * Math.sin((k + shift) / 6) * Math.cos(r / 3));
        grid.cells[i] = i % 4 === 0 ? 0x2580 : 32;
      }
    }
  };
  fill();
  const renderers: [ReturnType<typeof gp.makeRenderer>, HTMLOutputElement | null][] = [];
  for (const canvas of document.querySelectorAll<HTMLCanvasElement>("[data-backs]")) {
    const name = canvas.dataset["backs"] === "atlas" ? "atlas" : "text";
    const r = gp.makeRenderer(name, canvas);
    r.setPalette(palette, false);
    r.resize(cols, rows, cw);
    Object.assign(canvas.style, { width: `${(cols * cw) / dpr}px`, height: `${(rows * cw * 2) / dpr}px` });
    renderers.push([r, document.querySelector<HTMLOutputElement>(`[data-backs-stats="${name}"]`)]);
  }
  const draw = (): void => {
    for (const [r, out] of renderers) {
      const s = r.draw(grid);
      if (out) out.textContent = `last draw: ${s.cells} cells, ${s.backImage ? "the backs as one image" : "cell by cell"}`;
    }
  };
  draw();
  $("#backs-shift").addEventListener("click", () => {
    shift += 7;
    fill();
    draw();
  });
  $("#backs-one").addEventListener("click", () => {
    grid.backs[cols + 1] = grid.backs[cols + 1] === 159 ? 32 : 159;
    draw();
  });
}

/** Case 25: candles on ohlcv-days, scrubbed with a finger. */
function candles(): void {
  const el = plot("candles-chart");
  const d = ohlcvDays();
  el.data = d;
  const day = (i: number): string => d.t[i]?.toISOString().slice(0, 10) ?? "?";
  $("#candles-status").textContent = `${d.t.length} days, ${day(0)} to ${day(d.t.length - 1)}; limit down on ${day(LIMIT_DOWN_ROW)} (row ${LIMIT_DOWN_ROW}); no trades on ${day(ZERO_VOLUME_ROW)} (row ${ZERO_VOLUME_ROW})`;
  const log = logger($("#candles-log"));
  for (const t of ["plot-hover", "plot-view"]) el.addEventListener(t, (e) => log(short(t, (e as CustomEvent).detail)));
}

/** Cases 26 and 27: each candle mode on the last 16 days, and the volume pane around the limit-down day. */
function candleModes(): void {
  const d = ohlcvDays();
  const at = (i: number): number => d.t[i]?.getTime() ?? 0;
  const n = d.t.length;
  for (const id of ["mode-candle", "mode-hollow", "mode-ohlc"]) {
    const el = plot(id);
    el.data = d;
    el.view = { x: { domain: [at(n - 16) - DAY / 2, at(n - 1) + DAY / 2] } };
  }
  for (const id of ["volume-on", "volume-off"]) {
    const el = plot(id);
    el.data = d;
    el.view = { x: { domain: [at(LIMIT_DOWN_ROW - 10), at(ZERO_VOLUME_ROW + 10)] } };
  }
}

/** Case 28: bars vertical, horizontal, grouped and stacked, with negative values. */
function bars(): void {
  const category = ["North Atlantic distribution centre", "Gulf coast", "Great Lakes", "Pacific northwest", "Mid-Atlantic", "Caribbean port"];
  const change = [12.5, -5.25, 8, -11.75, 3.125, 7.5];
  for (const id of ["bar-vertical", "bar-horizontal"]) plot(id).data = { category, change };
  const three = { category: category.slice(0, 5), north: [14, 9, -6, 11, 4], south: [-8, 12, 7, -3, 10], west: [5, -4, 9, 6, -12] };
  for (const id of ["bar-grouped", "bar-stacked"]) plot(id).data = three;
}

/** Case 29: three sparklines fed one stream through append(). */
function sparklines(): void {
  const ids = ["spark-line", "spark-area", "spark-column"];
  const r = rng(73);
  const x: number[] = [];
  const y: number[] = [];
  let w = 50;
  for (let i = 0; i < 60; i++) {
    w += (r() - 0.5) * 4;
    x.push(i);
    y.push(Math.round(w * 100) / 100);
  }
  for (const id of ids) plot(id).data = { x, cpu: y };
  const status = $("#spark-status");
  let i = x.length;
  let timer: ReturnType<typeof setInterval> | undefined;
  const tick = (): void => {
    w += (r() - 0.5) * 4;
    const v = Math.round(w * 100) / 100;
    for (const id of ids) plot(id).append({ x: i, cpu: v });
    status.textContent = `${i + 1} rows; last appended x ${i}, cpu ${v}`;
    i++;
  };
  $("#spark-start").addEventListener("click", () => {
    if (timer === undefined) timer = setInterval(tick, 200);
  });
  $("#spark-stop").addEventListener("click", () => {
    clearInterval(timer);
    timer = undefined;
    status.textContent = `stopped at ${i} rows`;
  });
}

/** `plot-data`'s detail (REQ-STREAM-070). */
interface DataDetail {
  rows: number;
  dropped: number;
  late: number;
  version: number;
}

/** Case 31: a live line on the simulated walk, following its newest sample. */
function liveLine(): void {
  const el = plot("live-line-chart");
  const status = $("#live-line-status");
  const feed = new Feed(31, 20);
  const t0 = Date.now() - 1500 * 20;
  const t: Date[] = [];
  const price: number[] = [];
  for (let i = 0; i < 1500; i++) {
    t.push(new Date(t0 + i * 20));
    price.push(feed.sample());
  }
  el.data = { t, price };
  const last = t[t.length - 1]?.getTime() ?? 0;
  el.view = { x: { domain: [last - 30_000, last], follow: true } };
  let dropped = 0;
  let late = 0;
  el.addEventListener("plot-data", (e) => {
    const d = (e as CustomEvent<DataDetail>).detail;
    dropped += d.dropped;
    late += d.late;
    const x = el.view["x"] as { follow?: boolean } | undefined;
    status.textContent = `simulated: ${d.rows} rows held, ${dropped} dropped by retain.rows, ${late} late, data version ${d.version}; follow ${x?.follow ? "on" : "off"}`;
  });
  let timer: ReturnType<typeof setInterval> | undefined;
  $("#live-line-start").addEventListener("click", () => {
    timer ??= setInterval(() => el.append({ t: new Date(), price: feed.sample() }), 20);
  });
  $("#live-line-stop").addEventListener("click", () => {
    clearInterval(timer);
    timer = undefined;
  });
}

/** Case 32: `paused`, Space and `announce` on a simulated stream, beside a chart without `announce`. */
function pauseAnnounce(): void {
  const els = [plot("announce-chart"), plot("silent-chart")];
  const status = $("#announce-status");
  const feed = new Feed(32, 100);
  const start = Date.now() - 300 * 100;
  const t: Date[] = [];
  const value: number[] = [];
  for (let i = 0; i < 300; i++) {
    t.push(new Date(start + i * 100));
    value.push(feed.sample());
  }
  for (const el of els) el.data = { t, value };
  const show = (): void => {
    const a = els[0] as GlyphPlot;
    const live = a.querySelector("[aria-live]");
    status.textContent = `simulated; ${a.paused ? "paused" : "live"}; live region: ${live ? `"${live.textContent ?? ""}"` : "none"}; the chart without announce has ${els[1]?.querySelector("[aria-live]") ? "a" : "no"} live region`;
  };
  for (const el of els) {
    el.addEventListener("plot-data", show);
    new MutationObserver(show).observe(el, { attributes: true, attributeFilter: ["paused"] });
  }
  let timer: ReturnType<typeof setInterval> | undefined;
  $("#announce-start").addEventListener("click", () => {
    timer ??= setInterval(() => {
      const row = { t: new Date(), value: feed.sample() };
      for (const el of els) el.append(row);
    }, 100);
  });
  $("#announce-stop").addEventListener("click", () => {
    clearInterval(timer);
    timer = undefined;
  });
  const pause = $("#announce-pause");
  pause.addEventListener("click", () => {
    const on = !(els[0] as GlyphPlot).paused;
    for (const el of els) el.paused = on;
    pause.setAttribute("aria-pressed", String(on));
    pause.textContent = on ? "Resume" : "Pause";
  });
}

// ------------------------------------------------------------ start

showEnv();
addMarks();
$("#copy").addEventListener("click", () => void copy());
$("#clear").addEventListener("click", () => {
  for (const k of Object.keys(marks)) delete marks[k];
  save();
  location.reload();
});
modes();
gaps();
million();
mono();
lightDark();
themeSwitch();
wideLabels();
resize();
script();
interaction();
lazy();
alias();
typeChange();
fit();
link();
brushLegend();
timeZone();
candles();
candleModes();
bars();
sparklines();
liveLine();
pauseAnnounce();
void backs().catch((e: unknown) => console.error(`test.html: back colours case: ${String(e)}`));
void mounts().catch((e: unknown) => console.error(`test.html: mount cases: ${String(e)}`));
void panic().catch((e: unknown) => console.error(`test.html: panic case: ${String(e)}`));
window.gpReady = true;

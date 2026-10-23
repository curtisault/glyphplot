// The site's entry (REQ-SITE-010): defines <glyph-plot> and <gp-example>,
// reads the flags (the charts' metas, modules and budgets through the shell's
// manifest(), which is web/pkg/modules.js; the modules' gzipped sizes from
// web/pkg/sizes.json; the gallery from site/examples/manifest.json with each
// example's seed from its module) and starts the Elm app. Nothing here loads
// wasm: each <glyph-plot> loads its family's module when it nears the screen.
//
// The ports do what Elm cannot: the theme on <html> and in localStorage
// (REQ-SITE-060), an element's methods and typed-array data, the clipboard,
// and an example's module source, read through Vite's `?raw` import.

/// <reference types="vite/client" />
import "./site.css";
import "glyphplot/element";
import type { GlyphPlot } from "glyphplot/element";
import { manifest } from "glyphplot";
// Defines <gp-example slug="…">: its build-time still, then its live chart.
import "../examples/element.ts";
import { examples } from "../examples/index.ts";
import entries from "../examples/manifest.json";
import { Elm } from "./Main.elm";

/** The key the reader's theme choice is kept under; index.html's inline script reads it too. */
const THEME_KEY = "glyphplot-theme";

/** Each example module's source text, loaded on its page only (Vite's `?raw`, one chunk an example). */
const sources = import.meta.glob<string>(["../examples/*.ts", "!../examples/api.ts", "!../examples/index.ts", "!../examples/element.ts"], {
  query: "?raw",
  import: "default",
});

/** web/pkg/sizes.json (gzip -9 of each module's wasm, written by `mise run build:wasm`); empty before a build. */
const sizes = Object.values(import.meta.glob<Record<string, number>>("../../web/pkg/sizes.json", { eager: true, import: "default" }))[0] ?? {};

function storedTheme(): string | null {
  try {
    const v = localStorage.getItem(THEME_KEY);
    return v === "light" || v === "dark" ? v : null;
  } catch {
    return null;
  }
}

const dark = matchMedia("(prefers-color-scheme: dark)");

/** The reader chose `theme`, or "system" to follow prefers-color-scheme again (REQ-SITE-060). */
function setTheme(theme: string): void {
  const root = document.documentElement;
  if (theme === "light" || theme === "dark") root.dataset["theme"] = theme;
  else delete root.dataset["theme"];
  try {
    if (theme === "light" || theme === "dark") localStorage.setItem(THEME_KEY, theme);
    else localStorage.removeItem(THEME_KEY);
  } catch {
    // storage blocked: the choice holds for this page only
  }
}

/** Runs `f` on the element of this id once Elm has rendered it: a port message can arrive before the next view. */
function withPlot(id: string, f: (el: GlyphPlot) => void, tries = 60): void {
  const el = document.getElementById(id);
  if (el && el.localName === "glyph-plot") f(el as GlyphPlot);
  else if (tries > 0) requestAnimationFrame(() => withPlot(id, f, tries - 1));
}

/** A seeded walk of n rows as two Float64Array columns (xorshift32, as site/src/Data.elm). */
function walk(n: number, seed: number): { x: Float64Array; walk: Float64Array } {
  const x = new Float64Array(n);
  const y = new Float64Array(n);
  let s = seed >>> 0 || 2463534242;
  let w = 0;
  for (let i = 0; i < n; i++) {
    s ^= s << 13;
    s >>>= 0;
    s ^= s >>> 17;
    s ^= s << 5;
    s >>>= 0;
    w += (s / 4294967296 - 0.5) * 2;
    x[i] = i;
    y[i] = w;
  }
  return { x, walk: y };
}

interface PlotCall {
  id: string;
  method: string;
  args: unknown;
}

const seeds = new Map(examples.map((e) => [e.slug, e.seed]));
const m = await manifest();

const app = Elm.Main.init({
  flags: {
    charts: m.charts,
    modules: m.modules,
    sizes,
    examples: (entries as { slug: string }[]).map((e) => ({ ...e, seed: seeds.get(e.slug) ?? null })),
    base: import.meta.env.BASE_URL,
    storedTheme: storedTheme(),
    systemTheme: dark.matches ? "dark" : "light",
  },
});

app.ports.setTheme.subscribe(setTheme);
dark.addEventListener("change", () => app.ports.systemTheme.send(dark.matches ? "dark" : "light"));

app.ports.plotCall.subscribe(({ id, method, args }: PlotCall) =>
  withPlot(id, (el) => {
    const reply = (value: string): void => app.ports.plotReply.send({ id, method, value });
    switch (method) {
      case "resetView":
        el.resetView();
        break;
      case "setView":
        el.view = args as Record<string, unknown>;
        break;
      case "text":
        el.ready.then(() => reply(el.text()), (e: unknown) => reply(`no frame: ${String(e)}`));
        break;
      case "fill": {
        const { n, seed } = args as { n: number; seed: number };
        const t0 = performance.now();
        const data = walk(n, seed);
        const made = performance.now() - t0;
        el.ready.then(() => {
          let t1 = 0;
          el.addEventListener(
            "plot-data",
            () => {
              const ms = performance.now() - t1;
              reply(`${n.toLocaleString("en-US")} rows set: ${made.toFixed(1)} ms to generate them, ${ms.toFixed(1)} ms from setting el.data to its plot-data event, measured now in this browser with performance.now().`);
            },
            { once: true },
          );
          el.view = null;
          t1 = performance.now();
          el.data = data;
        });
        break;
      }
    }
  }),
);

app.ports.copy.subscribe(({ id, text }: { id: string; text: string }) => {
  navigator.clipboard.writeText(text).then(
    () => app.ports.copied.send({ id, ok: true }),
    () => app.ports.copied.send({ id, ok: false }),
  );
});

app.ports.requestSource.subscribe((slug: string) => {
  const load = sources[`../examples/${slug}.ts`];
  if (!load) {
    app.ports.gotSource.send({ slug, text: null });
    return;
  }
  load().then(
    (text) => app.ports.gotSource.send({ slug, text }),
    () => app.ports.gotSource.send({ slug, text: null }),
  );
});

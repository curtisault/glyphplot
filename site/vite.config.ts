// Vite for site/: `glyphplot` and `glyphplot/element` alias web/src (the
// plan's architecture diagram), so the pages run the shell's TypeScript as it
// is and its wasm from web/pkg, whichever build `mise run build:wasm` or
// `mise run build:wasm:debug` left there. Pages: index.html (the Elm site,
// REQ-SITE-010: its routes are paths, which `vite` and `vite preview` answer
// with index.html as a single-page app), test.html (the owner's checklist,
// REQ-TEST-040), blank.html (the browser tests' bare page), bench/render.html
// (the renderer bench, REQ-RENDER-010) and the pages bench:web and bench:load
// drive (bench/web.html, bench/load.html).
//
// The Elm app is compiled by `elmPlugin` (elm.mjs) with the `elm` binary mise
// pins (no npm package): `elm make --optimize` for a build, plain `elm make`
// under `vite`, its output wrapped as an ES module exporting `Elm`, and in a
// build put in a chunk of its own (assets/elm-*.js) so its size reads alone.
//
// Under `vite` (mise run dev), a change under web/pkg (a debug rebuild
// swapped in by scripts/dev.sh's watcher) reloads every open page
// (REQ-BUILD-080). The checklist's live candlestick has a page of its own,
// live.html, so its network panel shows the one module it plays.
//
// `src`'s sample files (REQ-DATA-150) are written by the build from the
// tests' walk: data/walk.csv, .json, .ndjson and .arrows (an Arrow IPC
// stream from web/test/support/arrow.mjs), served by `vite` and in site/dist.

import { defineConfig, type Plugin, type ViteDevServer } from "vite";
import { arrowStream, walk } from "../web/test/support/arrow.mjs";
import { elmPlugin, siteBase } from "./elm.mjs";

const dir = (rel: string): string => decodeURIComponent(new URL(rel, import.meta.url).pathname).replace(/\/$/, "");
const site = dir(".");
const repo = dir("..");
const src = dir("../web/src");
const pkg = dir("../web/pkg");

/** Reloads the pages when web/pkg changes: the build swaps the folder in whole, so any event under it counts. */
function reloadOnWasm(): Plugin {
  return {
    name: "glyphplot:reload-on-wasm",
    apply: "serve",
    configureServer(server: ViteDevServer) {
      server.watcher.add(pkg);
      let timer: ReturnType<typeof setTimeout> | undefined;
      server.watcher.on("all", (_event: string, file: string) => {
        if (!file.startsWith(`${pkg}/`) && file !== pkg) return;
        clearTimeout(timer);
        timer = setTimeout(() => {
          server.moduleGraph.invalidateAll();
          server.ws.send({ type: "full-reload", path: "*" });
          server.config.logger.info("glyphplot: web/pkg changed, pages reloaded", { timestamp: true });
        }, 300);
      });
    },
  };
}

/** The sample files `src` reads in the browser tests and the checklist, by path under the site. */
function sampleFiles(): Record<string, string | Uint8Array> {
  const w = walk(50);
  const iso = w.t.map((t) => new Date(t).toISOString());
  return {
    "data/walk.csv": `t,y\n${iso.map((t, i) => `${t},${w.y[i]}`).join("\n")}\n`,
    "data/walk.json": JSON.stringify({ t: iso, y: w.y }),
    "data/walk.ndjson": `${iso.map((t, i) => JSON.stringify({ t, y: w.y[i] })).join("\n")}\n`,
    "data/walk.arrows": arrowStream({ t: { type: "timestamp", values: w.t }, y: { type: "f64", values: w.y } }),
  };
}

/** Emits the sample files into the build and serves them under `vite`. */
function sampleData(): Plugin {
  return {
    name: "glyphplot:sample-data",
    generateBundle() {
      for (const [fileName, source] of Object.entries(sampleFiles())) this.emitFile({ type: "asset", fileName, source });
    },
    configureServer(server: ViteDevServer) {
      server.middlewares.use((req, res, next) => {
        const path = ((req as { url?: string }).url ?? "").split("?")[0]?.replace(/^\//, "") ?? "";
        const body = sampleFiles()[path];
        if (body === undefined) return next();
        res.end(body);
      });
    },
  };
}

export default defineConfig({
  root: site,
  // The Elm site's routes are paths (/line, /docs/…, /examples/…), so assets
  // sit under an absolute base: `/`, or GLYPHPLOT_BASE (`/glyphplot/`) for a
  // site served from a folder. main.ts gives it to Elm, which prefixes every link.
  base: siteBase(),
  resolve: {
    alias: [
      { find: /^glyphplot$/, replacement: `${src}/index.ts` },
      { find: /^glyphplot\/element$/, replacement: `${src}/element.ts` },
      // The shell's own modules, for the renderer bench only (not the package's API).
      { find: /^glyphplot\/src\/(.*)$/, replacement: `${src}/$1` },
    ],
  },
  server: {
    port: 5310,
    // web/src, web/pkg and web/fonts sit outside site/.
    fs: { allow: [repo] },
  },
  preview: { port: 4310, strictPort: true },
  build: {
    outDir: "dist",
    emptyOutDir: true,
    target: "es2022",
    rollupOptions: {
      input: {
        index: `${site}/index.html`,
        test: `${site}/test.html`,
        live: `${site}/live.html`,
        blank: `${site}/blank.html`,
        render: `${site}/bench/render.html`,
        "bench-web": `${site}/bench/web.html`,
        "bench-load": `${site}/bench/load.html`,
      },
      output: {
        // The Elm app in a chunk of its own, so its size reads alone (assets/elm-*.js).
        codeSplitting: { groups: [{ name: "elm", test: /\.elm$/ }] },
      },
    },
  },
  plugins: [elmPlugin(site), reloadOnWasm(), sampleData()],
});

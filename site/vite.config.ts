// Vite for site/: `glyphplot` and `glyphplot/element` alias web/src (the
// plan's architecture diagram), so the pages run the shell's TypeScript as it
// is and its wasm from web/pkg, whichever build `mise run build:wasm` or
// `mise run build:wasm:debug` left there. Pages: test.html (the owner's
// checklist, REQ-TEST-040), blank.html (the browser tests' bare page) and
// bench/render.html (the renderer bench, REQ-RENDER-010).
//
// Under `vite` (mise run dev), a change under web/pkg (a debug rebuild
// swapped in by scripts/dev.sh's watcher) reloads every open page
// (REQ-BUILD-080).

import { defineConfig, type Plugin, type ViteDevServer } from "vite";

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

export default defineConfig({
  root: site,
  // Relative URLs, so the built pages work from any folder.
  base: "./",
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
        blank: `${site}/blank.html`,
        render: `${site}/bench/render.html`,
      },
    },
  },
  plugins: [reloadOnWasm()],
});

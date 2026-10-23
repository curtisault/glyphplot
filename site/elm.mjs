// The Elm site's build pieces for vite.config.ts, in plain JavaScript so the
// site's tsc needs no Node types (elm.d.mts types them).
//
// `elmPlugin()` compiles an imported `.elm` module with `elm make` (`$ELM`,
// else the `elm` that mise pins and puts on the path; no npm package):
// `--optimize` for a build, plain under `vite`. Elm's output is a script that
// sets `this.Elm`; the module runs it with `this` bound to a local object and
// exports `Elm`. Under `vite` a change to any .elm file reloads the pages.
//
// `siteBase()` is the base the site is served under: `/`, or GLYPHPLOT_BASE
// (`/glyphplot/`) for a site served from a folder.

import { execFileSync } from "node:child_process";
import { mkdirSync, readdirSync, readFileSync } from "node:fs";

/** @returns {string} */
export function siteBase() {
  return process.env.GLYPHPLOT_BASE ?? "/";
}

/**
 * The Elm sources under `dir`, watched so a change to any module recompiles Main.elm.
 * @param {string} dir
 * @returns {string[]}
 */
function elmSources(dir) {
  return readdirSync(dir, { recursive: true })
    .map(String)
    .filter((f) => f.endsWith(".elm"))
    .map((f) => `${dir}/${f}`);
}

/**
 * @param {string} site the folder holding elm.json
 * @returns {import("vite").Plugin}
 */
export function elmPlugin(site) {
  let optimize = false;
  return {
    name: "glyphplot:elm",
    enforce: "pre",
    configResolved(config) {
      optimize = config.command === "build";
    },
    load(id) {
      const file = id.split("?")[0] ?? id;
      if (!file.endsWith(".elm")) return null;
      for (const f of elmSources(`${site}/src`)) this.addWatchFile(f);
      mkdirSync(`${site}/elm-stuff`, { recursive: true });
      const out = `${site}/elm-stuff/glyphplot-site.js`;
      try {
        execFileSync(process.env.ELM ?? "elm", ["make", file, `--output=${out}`, ...(optimize ? ["--optimize"] : [])], {
          cwd: site,
          stdio: ["ignore", "pipe", "pipe"],
        });
      } catch (e) {
        throw new Error(`elm make ${file}:\n${e.stderr?.toString() || e.stdout?.toString() || e.message}`);
      }
      const js = readFileSync(out, "utf8");
      return { code: `const scope = {};\n(function () {\n${js}\n}).call(scope);\nexport const Elm = scope.Elm;\n`, map: null };
    },
    handleHotUpdate({ file, server }) {
      if (!file.endsWith(".elm")) return;
      server.moduleGraph.invalidateAll();
      server.ws.send({ type: "full-reload", path: "*" });
      return [];
    },
  };
}

// The benches' static server (REQ-BENCH-020): site/dist, the release build
// `mise run build:site` makes, at `/`, and the fixtures `cargo run -p
// glyphplot --example fixtures -- write target/fixtures` writes at
// `/fixtures/`. It serves as a static host would: HTML, JS, CSS, JSON, SVG
// and wasm gzip -9 when the request accepts gzip (the level `mise run size`
// gates), each body compressed once and kept; fonts, images and the
// fixtures' raw f64 as they are. Every response is cross-origin isolated
// (COOP same-origin, COEP require-corp), so performance.now() is precise to
// 5 µs rather than 100 µs, and `no-store`, so no load is served from cache.
//
//   import { serve } from "./serve.mjs"; const s = await serve(); … s.url … await s.close();
//   node bench/serve.mjs [port]      serve until Ctrl-C

import { createServer } from "node:http";
import { existsSync, readFileSync, statSync } from "node:fs";
import { extname, join, normalize, resolve, sep } from "node:path";
import { fileURLToPath } from "node:url";
import { gzipSync } from "node:zlib";

const here = fileURLToPath(new URL(".", import.meta.url));
export const SITE_DIST = resolve(here, "../dist");
export const REPO = resolve(here, "../..");
export const FIXTURES = resolve(REPO, "target/fixtures");

const TYPES = {
  ".html": "text/html; charset=utf-8",
  ".js": "text/javascript; charset=utf-8",
  ".mjs": "text/javascript; charset=utf-8",
  ".css": "text/css; charset=utf-8",
  ".json": "application/json",
  ".svg": "image/svg+xml",
  ".wasm": "application/wasm",
  ".woff2": "font/woff2",
  ".png": "image/png",
  ".f64": "application/octet-stream",
};
const COMPRESS = new Set([".html", ".js", ".mjs", ".css", ".json", ".svg", ".wasm"]);

/** The file a URL path names under `root`, or null outside it or missing. */
function fileIn(root, path) {
  const file = normalize(join(root, path));
  if (file !== root && !file.startsWith(root + sep)) return null;
  if (!existsSync(file)) return null;
  const st = statSync(file);
  if (st.isDirectory()) return existsSync(join(file, "index.html")) ? join(file, "index.html") : null;
  return file;
}

/**
 * Starts the server on `port` (0: any free one); resolves to its URL, a
 * `close`, and `page(path, html)`, which serves `html` at `path` as if it
 * were a file of dist (bench:load's page with preload hints).
 */
export function serve({ port = 0, dist = SITE_DIST, fixtures = FIXTURES } = {}) {
  const gz = new Map();
  const pages = new Map();
  const server = createServer((req, res) => {
    const path = decodeURIComponent(new URL(req.url ?? "/", "http://x").pathname);
    const made = pages.get(path);
    if (made) {
      const body = /\bgzip\b/.test(String(req.headers["accept-encoding"] ?? "")) ? made.gz : made.raw;
      res.writeHead(200, {
        "content-type": TYPES[".html"],
        "cross-origin-opener-policy": "same-origin",
        "cross-origin-embedder-policy": "require-corp",
        "cache-control": "no-store",
        vary: "accept-encoding",
        ...(body === made.gz ? { "content-encoding": "gzip" } : {}),
        "content-length": body.length,
      });
      res.end(req.method === "HEAD" ? undefined : body);
      return;
    }
    const file = path.startsWith("/fixtures/") ? fileIn(fixtures, path.slice("/fixtures/".length)) : fileIn(dist, path.replace(/^\/+/, ""));
    if (!file) {
      res.writeHead(404, { "content-type": "text/plain" }).end("not found");
      return;
    }
    const ext = extname(file);
    const headers = {
      "content-type": TYPES[ext] ?? "application/octet-stream",
      "cross-origin-opener-policy": "same-origin",
      "cross-origin-embedder-policy": "require-corp",
      "cross-origin-resource-policy": "same-origin",
      "cache-control": "no-store",
      vary: "accept-encoding",
    };
    let body;
    if (COMPRESS.has(ext) && /\bgzip\b/.test(String(req.headers["accept-encoding"] ?? ""))) {
      const key = `${file}:${statSync(file).mtimeMs}`;
      body = gz.get(key);
      if (!body) {
        body = gzipSync(readFileSync(file), { level: 9 });
        gz.set(key, body);
      }
      headers["content-encoding"] = "gzip";
    } else body = readFileSync(file);
    headers["content-length"] = body.length;
    res.writeHead(200, headers);
    res.end(req.method === "HEAD" ? undefined : body);
  });
  return new Promise((ok, fail) => {
    server.once("error", fail);
    server.listen(port, "127.0.0.1", () => {
      const a = server.address();
      ok({
        url: `http://localhost:${typeof a === "object" && a ? a.port : port}`,
        close: () => new Promise((done) => server.close(() => done())),
        page: (p, html) => {
          const raw = Buffer.from(html);
          pages.set(p, { raw, gz: gzipSync(raw, { level: 9 }) });
        },
      });
    });
  });
}

if (process.argv[1] && fileURLToPath(import.meta.url) === resolve(process.argv[1])) {
  const s = await serve({ port: Number(process.argv[2] ?? 5313) });
  console.log(`serving ${SITE_DIST} and /fixtures/ from ${FIXTURES} on ${s.url} (gzip -9); Ctrl-C stops`);
}

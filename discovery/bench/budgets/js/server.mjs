// Throwaway static server for the browser measurements: serves the bench
// directory (budgets/) with cross-origin isolation (so performance.now() is
// precise to 5 µs, not 100 µs), application/wasm, and gzip -9 bodies for
// .wasm and .js when the client accepts gzip, as a production host would.
//   node server.mjs [port]
import { createServer } from "node:http";
import { readFileSync, existsSync, statSync } from "node:fs";
import { gzipSync } from "node:zlib";
import { join, extname, normalize } from "node:path";
import { fileURLToPath } from "node:url";

const root = fileURLToPath(new URL("..", import.meta.url));
const types = { ".html": "text/html", ".js": "text/javascript", ".mjs": "text/javascript", ".wasm": "application/wasm", ".json": "application/json" };
const cache = new Map();
const port = Number(process.argv[2] ?? 8765);
createServer((req, res) => {
  const path = normalize(decodeURIComponent(new URL(req.url, "http://x").pathname)).replace(/^\/+/, "");
  const file = join(root, path || "js/page.html");
  if (!file.startsWith(root) || !existsSync(file) || statSync(file).isDirectory()) {
    res.writeHead(404).end();
    return;
  }
  const ext = extname(file);
  const headers = {
    "content-type": types[ext] ?? "application/octet-stream",
    "cross-origin-opener-policy": "same-origin",
    "cross-origin-embedder-policy": "require-corp",
    "cache-control": "no-store",
  };
  let body = readFileSync(file);
  if ((ext === ".wasm" || ext === ".js" || ext === ".mjs") && /gzip/.test(req.headers["accept-encoding"] ?? "")) {
    const key = file + statSync(file).mtimeMs;
    if (!cache.has(key)) cache.set(key, gzipSync(body, { level: 9 }));
    body = cache.get(key);
    headers["content-encoding"] = "gzip";
  }
  headers["content-length"] = body.length;
  res.writeHead(200, headers).end(body);
}).listen(port, () => console.log(`serving ${root} on http://localhost:${port}`));

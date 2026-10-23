// REQ-DATASET-030: the npm package (`npm pack --dry-run` of web/) holds no
// ShareAlike dataset of datasets/manifest.json, by path or by SHA-256.
// `mise run datasets:check` checks the published crates the same way.
import { test } from "node:test";
import assert from "node:assert/strict";
import { execFileSync } from "node:child_process";
import { join, relative } from "node:path";
import { ROOT, checkPackages, classify, readManifest } from "../../scripts/datasets.mjs";

test("the npm package holds no ShareAlike dataset", () => {
  const web = join(ROOT, "web");
  const packed = JSON.parse(execFileSync("npm", ["pack", "--dry-run", "--json", "--ignore-scripts"], { cwd: web, encoding: "utf8", stdio: ["ignore", "pipe", "pipe"] }));
  const files = packed.flatMap((p) => p.files.map((f) => relative(ROOT, join(web, f.path)).split("\\").join("/")));
  assert.ok(files.length > 0);
  const manifest = readManifest();
  // The manifest has a ShareAlike entry for the check to find (HYG).
  assert.ok(manifest.datasets.some((d) => classify(d.licence).kind === "sharealike"));
  const { problems } = checkPackages(manifest, [{ name: "npm glyphplot", files }]);
  assert.deepEqual(problems, []);
});

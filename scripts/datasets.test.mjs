// The dataset checks fail as REQ-DATASET-010, 020 and 030 say: each case
// builds a small repository in a temporary folder and runs the checks of
// scripts/datasets.mjs on it, packages given as lists.
import { test } from "node:test";
import assert from "node:assert/strict";
import { createHash } from "node:crypto";
import { mkdirSync, mkdtempSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { dirname, join } from "node:path";
import { checkAll, checkFields, classify, csvFields, hygSubset, readManifest, MAX_COMMITTED } from "./datasets.mjs";

const sha = (b) => createHash("sha256").update(b).digest("hex");
const noGit = { ignored: () => false, tracked: () => false };

/** A repository with `files` (path → content) and a manifest of `datasets`; returns its root and a cleanup. */
function repo(files, datasets) {
  const root = mkdtempSync(join(tmpdir(), "gp-datasets-"));
  // The test datasets' README the check reads: an empty table unless `files` gives one.
  mkdirSync(join(root, "crates/glyphplot/tests/data"), { recursive: true });
  writeFileSync(join(root, "crates/glyphplot/tests/data/README.md"), "| File | Source |\n|---|---|\n");
  for (const [p, body] of Object.entries(files)) {
    mkdirSync(dirname(join(root, p)), { recursive: true });
    writeFileSync(join(root, p), body);
  }
  mkdirSync(join(root, "datasets"), { recursive: true });
  writeFileSync(join(root, "datasets/manifest.json"), JSON.stringify({ datasets }));
  return { root, done: () => rmSync(root, { recursive: true, force: true }) };
}

/** An entry for `path` holding `body`. */
function entry(id, path, body, more = {}) {
  return {
    id,
    title: id,
    publisher: "Someone",
    url: "https://example.org/data",
    licence: "Public domain",
    licenceUrl: "https://example.org/terms",
    retrieved: "2026-10-23",
    storage: "committed",
    paths: [path],
    size: Buffer.byteLength(body),
    sha256: sha(Buffer.from(body)),
    ...more,
  };
}

function run(files, datasets, { packages = [], git = noGit } = {}) {
  const r = repo(files, datasets);
  try {
    return checkAll({ root: r.root, manifest: readManifest(r.root), packages, git }).problems;
  } finally {
    r.done();
  }
}

const has = (problems, re) => assert.ok(problems.some((p) => re.test(p)), `expected ${re} in:\n${problems.join("\n")}`);

test("the allow-list: public domain, CC0, CC BY, ODC-By, MIT, BSD; NonCommercial, NoDerivatives and proprietary refused", () => {
  for (const l of ["Public domain", "Public domain (U.S. government work)", "CC0 1.0", "CC-BY-4.0", "CC BY 4.0", "ODC-By 1.0", "MIT", "BSD-3-Clause", "BSD 2-Clause"]) assert.equal(classify(l).kind, "allowed", l);
  for (const [l, why] of [
    ["CC BY-NC 3.0 IGO", "NonCommercial"],
    ["CC BY-NC-SA 4.0", "NonCommercial"],
    ["Non-commercial use only", "NonCommercial"],
    ["CC BY-ND 4.0", "NoDerivatives"],
    ["Proprietary", "proprietary"],
    ["All rights reserved", "proprietary"],
  ]) {
    assert.deepEqual([classify(l).kind, classify(l).why], ["refused", why], l);
  }
  assert.equal(classify("CC BY-SA 4.0").kind, "sharealike");
  assert.equal(classify("Not stated").kind, "unknown");
  assert.equal(classify("GPL-3.0").kind, "unknown");
});

test("a clean repository passes", () => {
  const body = '{"a":[1,2,3]}';
  assert.deepEqual(run({ "datasets/a.json": body }, [entry("a", "datasets/a.json", body)]), []);
});

test("REQ-DATASET-010: a file in a data folder with no entry fails; notes and licences beside it do not", () => {
  const p = run({ "site/public/data/stray.csv": "x\n1\n", "site/public/data/README.md": "notes", "site/public/data/hyg/LICENSE.md": "notice" }, []);
  assert.equal(p.length, 1, p.join("\n"));
  has(p, /site\/public\/data\/stray\.csv: no entry/);
});

test("REQ-DATASET-010: a different hash or size fails", () => {
  const e = entry("a", "datasets/a.json", "[1,2,3]");
  const p = run({ "datasets/a.json": "[1,2,4]" }, [e]);
  has(p, /a: datasets\/a\.json has SHA-256 [0-9a-f]{64}, the manifest says/);
  const q = run({ "datasets/a.json": "[1,2,3,4]" }, [e]);
  has(q, /is 9 B, the manifest says 7 B/);
});

test("REQ-DATASET-010: a missing field or a missing committed file fails", () => {
  const e = entry("a", "datasets/a.json", "x");
  delete e.licenceUrl;
  has(checkFields({ datasets: [e] }), /no licenceUrl/);
  has(run({}, [entry("a", "datasets/a.json", "x")]), /a: datasets\/a\.json is missing/);
});

test("REQ-DATASET-020: a committed dataset over 1 MB fails", () => {
  const big = "x".repeat(MAX_COMMITTED + 1);
  has(run({ "datasets/big.csv": big }, [entry("big", "datasets/big.csv", big)]), /big: 1000001 B is over the 1000000 B \(1 MB\)/);
  const ok = "x".repeat(MAX_COMMITTED);
  assert.deepEqual(run({ "datasets/ok.csv": ok }, [entry("ok", "datasets/ok.csv", ok)]), []);
});

test("REQ-DATASET-020: NonCommercial, NoDerivatives and proprietary licences fail, named; an unknown committed licence fails", () => {
  const b = "1";
  has(run({ "datasets/g.csv": b }, [entry("gaia", "datasets/g.csv", b, { licence: "CC BY-NC 3.0 IGO" })]), /gaia: licence "CC BY-NC 3.0 IGO" is NonCommercial/);
  has(run({ "datasets/n.csv": b }, [entry("nd", "datasets/n.csv", b, { licence: "CC BY-ND 4.0" })]), /nd: .* is NoDerivatives/);
  has(run({ "datasets/x.csv": b }, [entry("ex", "datasets/x.csv", b, { licence: "Proprietary" })]), /ex: .* is proprietary/);
  has(run({ "datasets/u.csv": b }, [entry("u", "datasets/u.csv", b, { licence: "Not stated" })]), /u: licence "Not stated" is not on the allow-list/);
  // Refused even when not committed: such data is never served either.
  has(run({}, [{ ...entry("feed", "x", b, { licence: "NonCommercial", storage: "live" }), paths: undefined, size: undefined, sha256: undefined }]), /feed: .* is NonCommercial/);
});

test("REQ-DATASET-030: a committed ShareAlike dataset fails", () => {
  const b = "id,mag\n1,2\n";
  has(run({ "datasets/hyg.csv": b }, [entry("hyg", "datasets/hyg.csv", b, { licence: "CC BY-SA 4.0" })]), /hyg: licence "CC BY-SA 4.0" is ShareAlike: it is never committed/);
});

test("REQ-DATASET-030: a ShareAlike dataset in a package fails, by path and by hash under another name", () => {
  const b = "id,mag\n1,2\n";
  const hyg = entry("hyg", "site/public/data/hyg/hyg.csv", b, { licence: "CC BY-SA 4.0", storage: "build", fetch: "https://example.org/hyg.csv" });
  const git = { ignored: (p) => p.startsWith("site/public/data/hyg/"), tracked: () => false };
  const files = { "site/public/data/hyg/hyg.csv": b, "web/fonts/stars.csv": b };
  assert.deepEqual(run(files, [hyg], { git, packages: [{ name: "npm glyphplot", files: ["web/dist/index.js"] }] }), []);
  has(run(files, [hyg], { git, packages: [{ name: "crate glyphplot", files: ["site/public/data/hyg/hyg.csv"] }] }), /crate glyphplot: holds site\/public\/data\/hyg\/hyg\.csv, hyg under "CC BY-SA 4.0"/);
  has(run(files, [hyg], { git, packages: [{ name: "npm glyphplot", files: ["web/fonts/stars.csv"] }] }), /npm glyphplot: holds web\/fonts\/stars\.csv, hyg/);
  // A public-domain dataset in a package is allowed.
  const pd = entry("wti", "crates/glyphplot/tests/data/wti.json", "[1]");
  const readme = "| File | Source | Licence | Retrieved |\n|---|---|---|---|\n| `wti.json` | https://example.org/data | Public domain | 2026-10-23 |\n";
  assert.deepEqual(run({ "crates/glyphplot/tests/data/wti.json": "[1]", "crates/glyphplot/tests/data/README.md": readme }, [pd], { packages: [{ name: "crate glyphplot", files: ["crates/glyphplot/tests/data/wti.json"] }] }), []);
});

test("a build-time dataset must be gitignored and untracked", () => {
  const b = "1";
  const e = entry("h", "site/public/data/h.csv", b, { storage: "build", fetch: "https://example.org/h" });
  has(run({}, [e]), /h: site\/public\/data\/h\.csv is fetched at build time but not gitignored/);
  has(run({}, [e], { git: { ignored: () => true, tracked: () => true } }), /fetched at build time but tracked by git/);
  // Absent is only a note: the check runs before a fetch.
  assert.deepEqual(run({}, [e], { git: { ignored: () => true, tracked: () => false } }), []);
});

test("the test datasets' README must agree with the manifest", () => {
  const b = "[1]";
  const e = entry("wti", "crates/glyphplot/tests/data/wti.json", b);
  const row = (url, lic, date) => `| File | Source | Licence | Retrieved |\n|---|---|---|---|\n| \`wti.json\` | ${url} | ${lic} | ${date} |\n`;
  const files = (readme) => ({ "crates/glyphplot/tests/data/wti.json": b, "crates/glyphplot/tests/data/README.md": readme });
  assert.deepEqual(run(files(row("<https://example.org/data>", "Public domain (U.S. government work)", "2026-10-23")), [e]), []);
  has(run(files(row("https://example.org/other", "Public domain", "2026-10-23")), [e]), /does not give the manifest's URL/);
  has(run(files(row("https://example.org/data", "Public domain", "2026-10-22")), [e]), /does not give the manifest's retrieval date 2026-10-23/);
  has(run(files(row("https://example.org/data", "MIT", "2026-10-23")), [e]), /licence cell does not start with the manifest's "Public domain"/);
  has(run(files("| File |\n|---|\n"), [e]), /no row for crates\/glyphplot\/tests\/data\/wti\.json/);
});

test("the HYG subset: mag 6.5 and brighter, no Sun, seven columns, quoting kept", () => {
  assert.deepEqual(csvFields('1,"a, b","say ""hi""",'), ["1", "a, b", 'say "hi"', ""]);
  const csv = [
    '"id","hip","proper","ra","dec","mag","ci","con","x"',
    "0,,Sol,0.0,0.0,-26.7,0.656,,0",
    '1,1,"",0.1,1.0,9.1,0.48,Psc,1',
    "2,2,Sirius,6.752,-16.716,-1.44,0.009,CMa,2",
    '3,3,"Name, with comma",1.0,2.0,6.5,,Ori,3',
    "4,4,,1.0,2.0,,0.1,Ori,4",
  ].join("\n");
  assert.equal(hygSubset(csv), 'id,ra,dec,mag,ci,proper,con\n2,6.752,-16.716,-1.44,0.009,Sirius,CMa\n3,1.0,2.0,6.5,,"Name, with comma",Ori\n');
});

test("the repository's manifest has every field", () => {
  assert.deepEqual(checkFields(readManifest()), []);
});

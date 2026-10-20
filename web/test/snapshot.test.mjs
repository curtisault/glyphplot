// Every chart's wasm frames for every fixture equal its native snapshot
// (REQ-SNAP-030): the snapshot file is rebuilt from the module's frames as
// crates/glyphplot/tests/common/snap.rs writes it (the 80 × 24 frame's text,
// ink and back layers whole, the other frames as FNV-1a hashes), and must
// equal the file. The data is the fixtures as `cargo run -p glyphplot
// --example fixtures -- write` writes them (REQ-FIX-010).
import { test } from "node:test";
import assert from "node:assert/strict";
import { existsSync, readFileSync } from "node:fs";
import { join } from "node:path";
import { BUILD, ROOT, manifest, types, moduleOfType, views, textOf, fixtureIndex, fixtureData, commitColumns } from "./support/wasm.mjs";

const SNAPSHOTS = join(ROOT, "crates/glyphplot/tests/snapshots");
const NO_BACK = 255;
const VERSION = 1;

/** One row of a layer as runs (REQ-SNAP-020). */
function runs(row, back) {
  const out = [];
  for (let i = 0; i < row.length; ) {
    const v = row[i];
    let n = 1;
    while (i + n < row.length && row[i + n] === v) n++;
    out.push((back && v === NO_BACK ? ".." : v.toString(16).padStart(2, "0")) + (n > 1 ? String(n) : ""));
    i += n;
  }
  return out.join(" ");
}

/** FNV-1a 64 over cols, rows, glyphs (u32 LE), inks, backs: the module's hash. */
function fnv1a(g) {
  // Two 32-bit halves: h * 0x100000001b3 = h * 0x1b3 + (h << 40).
  let hi = 0xcbf29ce4;
  let lo = 0x84222325;
  const eat = (b) => {
    lo = (lo ^ b) >>> 0;
    const loMul = lo * 0x1b3;
    const carry = Math.floor(loMul / 0x100000000);
    const newLo = loMul >>> 0;
    const newHi = (hi * 0x1b3 + carry + ((lo << 8) >>> 0)) >>> 0;
    hi = newHi;
    lo = newLo;
  };
  const u32 = (v) => {
    eat(v & 0xff);
    eat((v >>> 8) & 0xff);
    eat((v >>> 16) & 0xff);
    eat((v >>> 24) & 0xff);
  };
  u32(g.cols);
  u32(g.rows);
  for (const c of g.cells) u32(c);
  for (const k of g.colors) eat(k);
  for (const b of g.backs) eat(b);
  return hi.toString(16).padStart(8, "0") + lo.toString(16).padStart(8, "0");
}

const label = (g, t, env, what) => `@ ${g.cols}x${g.rows} · t=${t} · ${env}${what ? ` · ${what}` : ""}`;

function whole(g, t, env, what, wideTail) {
  const out = [label(g, t, env, what)];
  for (const row of textOf(g, wideTail).split("\n")) out.push(`|${row}|`);
  out.push("ink");
  for (let r = 0; r < g.rows; r++) out.push(runs(g.colors.subarray(r * g.cols, (r + 1) * g.cols), false));
  out.push("back");
  for (let r = 0; r < g.rows; r++) out.push(runs(g.backs.subarray(r * g.cols, (r + 1) * g.cols), true));
  return out.join("\n") + "\n";
}

const hash = (g, t, env) => `${label(g, t, env)} · fnv1a ${fnv1a(g)}\n`;

const ENVS = { plain: [false, false], mono: [true, false], paper: [false, true] };

/** Draws a frame of `cols × rows` at `t` in `env` and returns the views after it, checked against text(). */
function draw(c, memory, wideTail, t, env, cols, rows) {
  const [mono, paper] = ENVS[env];
  c.resize(cols, rows);
  c.set_mono(mono);
  c.frame(t, paper);
  const text = c.text(); // may allocate: the views are made after it
  const g = views(c, memory);
  assert.equal(textOf(g, wideTail), text, "the views equal text() (REQ-WASM-030)");
  return g;
}

/** The snapshot jobs of a chart: each accepted fixture in the default mode, the other modes on sine-1k (or the first of more than two rows). */
function jobs(meta, fixtures) {
  const accepted = fixtures.filter((f) => meta.shapes.includes(f.shape));
  const out = accepted.map((f) => ({ fixture: f, mode: meta.modes[0], file: `${f.name}.snap` }));
  const modeFixture = accepted.find((f) => f.name === "sine-1k") ?? accepted.find((f) => f.rows > 2);
  if (modeFixture) for (const mode of meta.modes.slice(1)) out.push({ fixture: modeFixture, mode, file: `${mode}-${modeFixture.name}.snap` });
  return out;
}

/** The zoom the native snapshot recorded: `zoom: ctrl+wheel dy -3 at X,Y, wheel end, move to X,Y`. */
function zoomOf(text) {
  const m = /^@ 80x24 · t=0 · plain · (zoom: ctrl\+wheel dy -3 at ([\d.]+),([\d.]+), wheel end, move to ([\d.]+),([\d.]+))$/m.exec(text);
  return m ? { what: m[1], wx: Number(m[2]), wy: Number(m[3]), mx: Number(m[4]), my: Number(m[5]) } : null;
}

const index = fixtureIndex();

for (const meta of manifest.charts) {
  for (const job of jobs(meta, index.fixtures)) {
    test(`${BUILD}: ${meta.slug} ${job.file.replace(/\.snap$/, "")} in wasm equals its native snapshot (REQ-SNAP-030)`, async () => {
      const path = join(SNAPSHOTS, meta.slug, job.file);
      assert.ok(existsSync(path), `no snapshot ${path}: run \`mise run snap ${meta.slug}\``);
      const want = readFileSync(path, "utf8");
      const { glue, memory } = await moduleOfType(meta.slug);
      const wideTail = glue.wide_tail();
      const options = job.mode === meta.modes[0] ? "{}" : JSON.stringify({ mode: job.mode });
      const { descriptor, columns } = fixtureData(job.fixture);
      const make = () => {
        const c = new glue.Chart(meta.slug, options);
        try {
          commitColumns(c, memory, descriptor, columns);
        } catch {
          // The native snapshot ignores a refused set_data too; the chart draws without data.
        }
        return c;
      };
      let got = `# glyphplot snapshot ${VERSION} · ${meta.slug} · ${job.fixture.name} · mode ${job.mode} · options ${options} · default view\n`;
      const c = make();
      got += whole(draw(c, memory, wideTail, 0, "plain", 80, 24), 0, "plain", null, wideTail);
      for (const t of [1, 2.5, 6]) got += hash(draw(c, memory, wideTail, t, "plain", 80, 24), t, "plain");
      got += hash(draw(c, memory, wideTail, 0, "mono", 80, 24), 0, "mono");
      got += hash(draw(c, memory, wideTail, 0, "paper", 80, 24), 0, "paper");
      got += whole(draw(c, memory, wideTail, 0, "plain", meta.min.cols, meta.min.rows), 0, "plain", null, wideTail);
      got += hash(draw(c, memory, wideTail, 0, "plain", 320, 120), 0, "plain");
      c.free();
      const zoom = zoomOf(want);
      assert.ok(zoom, "the snapshot records its zoom");
      const z = make();
      draw(z, memory, wideTail, 0, "plain", 80, 24);
      z.input(4, zoom.wx, zoom.wy, 0, -3, 0, 2, 0); // ctrl+wheel dy -3
      z.input(4, zoom.wx, zoom.wy, 0, 0, 0, 2, 0); // the wheel's end
      z.input(0, zoom.mx, zoom.my, 0, 0, 0, 0, 0); // a hover move
      got += whole(draw(z, memory, wideTail, 0, "plain", 80, 24), 0, "plain", zoom.what, wideTail);
      z.free();
      if (got !== want) {
        const w = want.split("\n");
        const g = got.split("\n");
        let n = 0;
        while (n < w.length && w[n] === g[n]) n++;
        const block = g.slice(0, n).reverse().find((l) => l.startsWith("@ ")) ?? "the head";
        assert.fail(`line ${n + 1} (in ${block}): file ${JSON.stringify(w[n])}, wasm ${JSON.stringify(g[n])}`);
      }
    });
  }
}

test(`${BUILD}: every chart the build plays has snapshots`, () => {
  for (const t of types) assert.ok(existsSync(join(SNAPSHOTS, t)), t);
});

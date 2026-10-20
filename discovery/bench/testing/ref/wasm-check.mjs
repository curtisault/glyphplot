// The same comparisons as tests/reference.rs, against the wasm32 build of the
// ports (cargo build --release --target wasm32-unknown-unknown), with no
// wasm-bindgen: plain exports over linear memory.
import { readFileSync } from "node:fs";

const wasmPath = new URL("../target/wasm32-unknown-unknown/release/glyphplot_testing_bench.wasm", import.meta.url);
const { instance } = await WebAssembly.instantiate(readFileSync(wasmPath));
const { memory, alloc_f64, ticks_into, lttb_into } = instance.exports;
const dir = new URL("./out/", import.meta.url);
const read = (name) => readFileSync(new URL(name, dir), "utf8").trim().split("\n");
const parse = (s) => (s === "-0" ? -0 : Number(s));

const out = alloc_f64(4096);
const inputs = read("ticks-in.txt");
const expected = read("ticks-expected.txt");
let bad = 0;
for (let i = 0; i < inputs.length; i++) {
  const [a, b, c] = inputs[i].split(" ").map(Number);
  const n = ticks_into(a, b, c, out, 4096);
  const got = Array.from(new Float64Array(memory.buffer, out, n));
  const want = expected[i] ? expected[i].split(" ").map(parse) : [];
  if (got.length !== want.length || got.some((g, k) => !Object.is(g, want[k]))) {
    if (bad++ < 10) console.log(`ticks(${inputs[i]}): wasm ${got.join(",")}, d3 ${want.join(",")}`);
  }
}
console.log(`wasm: ${bad} of ${inputs.length} ticks cases differ from d3-array`);

let lbad = 0;
for (const line of read("lttb-expected.txt")) {
  const [name, threshold, ...want] = line.split(" ");
  const pts = read(`lttb-${name}.txt`).map((l) => l.split(" ").map(Number));
  const x = alloc_f64(pts.length), y = alloc_f64(pts.length), o = alloc_f64(+threshold);
  new Float64Array(memory.buffer, x, pts.length).set(pts.map((p) => p[0]));
  new Float64Array(memory.buffer, y, pts.length).set(pts.map((p) => p[1]));
  const n = lttb_into(x, y, pts.length, +threshold, o);
  const got = Array.from(new Float64Array(memory.buffer, o, n)).join(" ");
  if (got !== want.join(" ")) lbad++, console.log(`lttb ${name} ${threshold}: differs`);
}
console.log(`wasm: ${lbad} lttb cases differ from the reference`);
process.exitCode = bad || lbad ? 1 : 0;

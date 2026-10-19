// Runs the inputs examples/capture.rs wrote through the references:
// d3-array 3.2.4's ticks, and flot-downsample's LTTB (lttb-reference.js,
// MIT, sveinn-steinarsson/flot-downsample at af872e0, loaded as written with
// a stand-in for jQuery's plugin list). Writes ref/out/*-expected.txt.
import { readFileSync, writeFileSync } from "node:fs";
import { ticks } from "d3-array";

const dir = new URL("./out/", import.meta.url);
const num = (n) => (Object.is(n, -0) ? "-0" : String(n));

const cases = readFileSync(new URL("ticks-in.txt", dir), "utf8").trim().split("\n");
writeFileSync(
  new URL("ticks-expected.txt", dir),
  cases.map((l) => ticks(...l.split(" ").map(Number)).map(num).join(" ")).join("\n") + "\n",
);

// The reference as published: an IIFE over jQuery that registers a plugin.
const plugins = [];
const source = readFileSync(new URL("lttb-reference.js", import.meta.url), "utf8");
new Function("jQuery", source)({ plot: { plugins } });
const fakePlot = { hooks: { processRawData: [] } };
plugins[0].init(fakePlot);
const lttb = (data, threshold) => {
  const series = { data, downsample: { threshold } };
  fakePlot.hooks.processRawData[0](fakePlot, series);
  return series.data;
};

const out = [];
for (const name of ["sine-1k", "walk-10k", "walk-100k"]) {
  const data = readFileSync(new URL(`lttb-${name}.txt`, dir), "utf8").trim().split("\n").map((l) => l.split(" ").map(Number));
  for (const threshold of [3, 100, 640, 999]) {
    const kept = lttb(data, threshold);
    out.push(`${name} ${threshold} ${kept.map((p) => data.indexOf(p)).join(" ")}`);
  }
}
writeFileSync(new URL("lttb-expected.txt", dir), out.join("\n") + "\n");
console.log(`${cases.length} ticks cases, ${out.length} lttb cases written to ref/out`);

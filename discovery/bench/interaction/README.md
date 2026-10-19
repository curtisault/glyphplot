# interaction bench (throwaway)

This crate is throwaway. It exists only to measure what one pointer move costs for
`discovery/interaction.md`. It is not part of glyphplot, nothing depends on it,
and it can be deleted once the report has been read.

It draws a line chart of 1,000,000 sorted points in braille, with a y axis, an
x axis and a readout row, into a `cols × rows` grid of `u32` code points and
`u8` colours (unicel's grid shape). It then times each step of one pointer
event: the pointer in cells turned into a dot and a view-state cursor, the hit
test (a binary search on sorted x), the crosshair and readout drawn over a
cached base frame, and the same event drawing the whole frame again from the
data. A 1,000,000-point scatter gives the spatial-grid hit test (build and
query) and a linear scan to compare with. Each timing runs 5 times and prints
the min and the median per event.

The series come from a SplitMix64 generator. The recorded numbers were taken
with an earlier xorshift64* generator, so a rerun draws different data of the
same shape.

## Rerun

From `/Users/curtisault/projects/glyphplot`:

```
cd discovery/bench/interaction
mise exec -- cargo run --release            # native; optional arg: number of points
mise exec -- wasm-pack build --target web --release --out-dir pkg
mise exec -- node node/bench.mjs            # the same in wasm under Node's V8
```

Record `uptime` before and after: the numbers move with the machine's load.
The crate has its own empty `[workspace]`, so its `target/` and `pkg/` stay
inside this folder. The release profile is unicel's (opt-level "s", LTO, one
codegen unit, panic abort, strip; wasm-opt `-Os`).

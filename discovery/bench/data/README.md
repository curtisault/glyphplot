# discovery/bench/data: throwaway

This crate is throwaway. It measured what discovery/data.md reports and is not
part of glyphplot: nothing in `crates/` or `web/` may depend on it, and it may
be deleted once the plan is written. It is a standalone crate (its own empty
`[workspace]`, its own `target/`).

What it holds:

- `src/core.rs`: a CSV parser (numbers and RFC 3339 times), JSON column and row
  parsers (serde_json), a hand-written Arrow IPC stream reader (Float64, Int64,
  Timestamp; validity to NaN; no compression or dictionaries), the same through
  arrow-rs, a ring buffer, min-max, M4 and LTTB downsamplers, and planet
  positions from JPL's Keplerian elements and from VSOP87A.
- `src/lib.rs`: the wasm-bindgen boundary over it, every path behind its own
  Cargo feature so a build with one more feature measures what that one adds.
- `src/bin/native.rs`: writes the fixtures (1k, 100k, 1M rows of a random walk
  as CSV, JSON and Arrow IPC) to `target/fixtures`, then times the native side.
- `sizes.sh`: builds one module per feature set with wasm-pack as unicel builds
  its modules and prints raw and gzip -9 sizes.
- `bench.mjs`: times the wasm side in Node (V8 standing in for Chrome), each case
  on a fresh instance so the memory growth it reports is its own.
- `csv-js.mjs`: CSV parsed in JS, for comparison.
- `combine.py`: folds several runs' tables into one (min of mins, median of medians).

To rerun, from the repository root's toolchain:

```
cd discovery/bench/data
mise exec -- cargo run --release --features native --bin native > target/native-run1.md
mise exec -- sh sizes.sh
mise exec -- node bench.mjs > target/node-run1.md
mise exec -- node csv-js.mjs > target/csvjs-run1.md
python3 -I combine.py target/node-run*.md
```

Run each at least three times and record `uptime` before and after: the
numbers in discovery/data.md were taken on a shared machine.

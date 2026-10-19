# scales bench (throwaway)

This crate is throwaway. It was built on 2026-10-18 for `discovery/scales.md` and is not part of glyphplot. Nothing depends on it, and the build under `crates/` must not copy from it without review.

It measures two things:

1. What each formatting and date dependency adds to a minimal wasm-bindgen module, gzipped. One exported function, `run`, makes one real call into one dependency; the Cargo feature picks which. `base` formats two integers and a `&str` with `format!`, as any chart module does; every other variant is `base` plus its call.
2. Whether a hand-written port of d3's linear ticks, `nice`, SI tick labels and UTC calendar ticks matches d3, and whether labels fitted to a row of cells ever overlap.

## Rerun

From the repository root, with mise's toolchain:

```sh
cd discovery/bench/scales
mise exec -- sh sizes.sh                 # every variant: raw and gzip -9 bytes
mise exec -- sh sizes.sh hand jiff-civil # some variants
mise exec -- cargo test --release        # d3 fixtures and property checks
mise exec -- cargo test --release -- --nocapture   # with the counts printed
mise exec -- cargo run --release --example fit     # sample axes and fit_x timing
mise exec -- node timing.mjs             # Intl and wasm label timings (after sizes.sh)
mise exec -- node locale.mjs             # a d3-shaped locale derived from Intl, timed
```

`sizes.sh` builds with `wasm-pack build --target web --release` and the release profile and wasm-opt flags of `../animate-unicode/crates/wasm/Cargo.toml` (opt-level "s", lto, one codegen unit, panic abort, strip; `wasm-opt -Os`).

## The d3 fixtures

`fixtures/d3.jsonl` holds 803 cases captured from d3-scale 4.0.2, d3-array 3.2.4, d3-format 3.1.0, d3-time 3.1.0 and d3-time-format 4.1.0. To capture them again, install those packages outside the repository without running their scripts, then point the script at that directory:

```sh
mkdir -p /tmp/d3 && cd /tmp/d3 && echo '{"private":true}' > package.json
npm install --ignore-scripts d3-scale@4.0.2 d3-array@3.2.4 d3-format@3.1.0 d3-time@3.1.0 d3-time-format@4.1.0
cd -  # back to discovery/bench/scales
node fixtures/capture.mjs /tmp/d3 > fixtures/d3.jsonl
```

Files: `src/hand.rs` (the port and `fit_x`), `src/lib.rs` (the variants), `tests/d3.rs` (fixtures), `tests/props.rs` (properties), `examples/fit.rs`, `timing.mjs`, `locale.mjs`, `sizes.sh`. `target/` is ignored by the repository's `.gitignore`.

# discovery/bench/testing (throwaway)

This crate is throwaway. It exists only to measure what `discovery/testing.md`
reports: what a fixture set costs to generate, what reducing a million points
costs, whether ports of d3-array's `ticks` and of the LTTB reference match
their originals bit for bit natively and in wasm, and what proptest and
quickcheck cost to compile and run. It is not glyphplot code and nothing
should import it.

Everything runs through mise from the repository root's toolchain
(Rust 1.97, Node 26). Commands are run from this directory.

## Reference fixtures (d3-array 3.2.4, flot-downsample at af872e0)

    mise exec -- npm --prefix ref install
    mise exec -- cargo run -q --release --example capture   # writes ref/out/*-in.txt, lttb-*.txt
    mise exec -- node ref/capture.mjs                       # writes ref/out/*-expected.txt
    mise exec -- cargo test -q --release --test reference -- --nocapture
    mise exec -- cargo test -q --release --features naive-round --test reference -- --nocapture
    mise exec -- cargo build -q --release --target wasm32-unknown-unknown --lib
    mise exec -- node ref/wasm-check.mjs

`ref/lttb-reference.js` is `jquery.flot.downsample.js` from
https://github.com/sveinn-steinarsson/flot-downsample (MIT, commit af872e0),
unchanged; `ref/capture.mjs` loads it with a stand-in for jQuery.
`ref/out/` is generated and ignored.

## Property tests

    mise exec -- cargo test -q --release --features proptest,quickcheck
    mise exec -- cargo test -q --release --features proptest --test props_proptest -- --ignored
    mise exec -- cargo test -q --release --features quickcheck --test props_quickcheck -- --ignored

The `--ignored` tests are false on purpose, to compare the two crates'
shrunk counterexamples.

## Costs

    mise exec -- cargo run -q --release --example cost      # fixtures and reductions, 7 runs, min and median
    mise exec -- cargo run -q --release --example columns   # two equal column formulas that disagree
    mise exec -- ./compile-cost.sh                          # clean builds, dev and release, none / proptest / quickcheck

The unicel contract timings in the report were taken in `../animate-unicode`
with `cargo test -q --release -p unicel --test contract` (read-only use), and
the no-LTO variant with `CARGO_TARGET_DIR` pointing at `target/unicel-nolto`
here and `--config 'profile.release.lto=false' --config
'profile.release.codegen-units=16' --config 'profile.release.opt-level=3'`.

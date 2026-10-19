#!/bin/sh
# Builds each variant with wasm-pack (--target web --release; release profile
# and wasm-opt flags as unicel's) and prints raw and gzip -9 sizes in bytes of
# the .wasm and of the JS glue. Run from this directory:
#   mise exec -- sh sizes.sh [variant ...]
set -e
VARIANTS=${*:-"empty base fmt-float ryu num-format-en num-format-any chrono chrono-tz time jiff-utc jiff-civil jiff-tz icu-decimal icu-datetime js-intl hand hand-linear hand-time"}
printf "%-16s %10s %10s %10s\n" variant wasm wasm.gz js.gz
for v in $VARIANTS; do
  out=target/pkg-$v
  wasm-pack --quiet build --target web --release --no-pack --out-dir "$out" -- --no-default-features --features "$v" >/dev/null 2>&1 \
    || wasm-pack --quiet build --target web --release --out-dir "$out" -- --no-default-features --features "$v" >/dev/null
  w=$out/scales_bench_bg.wasm
  printf "%-16s %10s %10s %10s\n" "$v" "$(wc -c < $w | tr -d ' ')" "$(gzip -9 -c $w | wc -c | tr -d ' ')" "$(gzip -9 -c $out/scales_bench.js | wc -c | tr -d ' ')"
done

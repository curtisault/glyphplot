#!/bin/sh
# Builds one wasm module per feature set as unicel builds its modules and
# prints each one's raw and gzip -9 size. Run from this directory through mise:
#   mise exec -- sh sizes.sh
set -e
for f in base options options,json options,csv json,csv json csv arrowmin arrowrs arrow2 ring downsample kepler vsop all; do
  if [ "$f" = base ]; then feat=""; else feat="--features $f"; fi
  RUSTFLAGS='--cfg getrandom_backend="wasm_js"' wasm-pack --log-level error build --target web --release --no-typescript --out-dir "target/pkg/$(echo $f | tr , +)" --out-name bench . -- --no-default-features $feat >/dev/null
  w="target/pkg/$(echo $f | tr , +)/bench_bg.wasm"
  printf '%s\t%s\t%s\n' "$f" "$(wc -c < "$w" | tr -d ' ')" "$(gzip -9 -c "$w" | wc -c | tr -d ' ')"
done

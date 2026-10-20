#!/usr/bin/env bash
# What each ladder step's module is made of: builds it unstripped (profile
# size-names: release, names kept), lists every item's shallow size with
# twiggy, and sums them by owner (scripts/classify.mjs). Shares are of the
# unoptimised cargo output, before wasm-bindgen and wasm-opt, so they are
# proportions, not the release module's bytes.
# Run from the glyphplot root:  mise exec -- discovery/bench/budgets/scripts/breakdown.sh
set -euo pipefail
here="$(cd "$(dirname "$0")/.." && pwd)"
twiggy="$here/ladder/target/tools/bin/twiggy"
for s in "0-empty:" "1-line:line" "3-time:time" "4-serde:time,serde" "6-downsample:time,serde,candle,downsample"; do
  name="${s%%:*}"; feats="${s#*:}"
  args=(--no-default-features)
  [ -n "$feats" ] && args+=(--features "$feats")
  cargo build -q --manifest-path "$here/ladder/Cargo.toml" --lib --target wasm32-unknown-unknown --profile size-names "${args[@]}"
  w="$here/ladder/target/wasm32-unknown-unknown/size-names/ladder.wasm"
  echo "== $name ($(wc -c < "$w" | tr -d ' ') bytes unoptimised)"
  "$twiggy" top -n 100000 --format csv "$w" | node "$here/scripts/classify.mjs"
done

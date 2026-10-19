#!/usr/bin/env bash
# Builds each step of the size ladder as its own wasm-pack release build
# (--target web, unicel's profile and wasm-opt flags) into pkg/<step>/ and
# prints raw and gzip -9 bytes of the wasm and of the JS glue.
# Run from the glyphplot root:  mise exec -- discovery/bench/budgets/scripts/ladder.sh
set -euo pipefail
here="$(cd "$(dirname "$0")/.." && pwd)"
crate="$here/ladder"
out="$here/pkg"
steps=(
  "0-empty:"
  "1-line:line"
  "2-scale:scale"
  "2b-scale-fmtfloat:fmtfloat"
  "3-time:time"
  "4-serde:time,serde"
  "5-candle:time,serde,candle"
  "6-downsample:time,serde,candle,downsample"
  "x-line-downsample-noserde:time,downsample"
)
mkdir -p "$out"; echo '{"type":"module"}' > "$out/package.json" 2>/dev/null || true
printf '%-28s %10s %10s %10s %10s\n' step wasm_raw wasm_gz js_raw js_gz
for s in "${steps[@]}"; do
  name="${s%%:*}"; feats="${s#*:}"
  args=(--no-default-features)
  [ -n "$feats" ] && args+=(--features "$feats")
  wasm-pack --quiet build "$crate" --target web --release --no-pack --out-dir "$out/$name" --out-name ladder -- "${args[@]}" >/dev/null 2>&1
  w="$out/$name/ladder_bg.wasm"; j="$out/$name/ladder.js"
  printf '%-28s %10s %10s %10s %10s\n' "$name" "$(wc -c < "$w" | tr -d ' ')" "$(gzip -9c "$w" | wc -c | tr -d ' ')" "$(wc -c < "$j" | tr -d ' ')" "$(gzip -9c "$j" | wc -c | tr -d ' ')"
done

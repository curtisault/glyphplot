#!/usr/bin/env bash
# Builds the formatting module three ways for wasm32 and prints raw and gzip -9 sizes.
# Throwaway (see README.md). Run from this directory: mise exec -- bash sizes.sh
set -euo pipefail
for v in base icu icu_dt; do
  args=()
  [ "$v" != base ] && args=(--features "$v")
  rm -f "target/wasm-$v/wasm32-unknown-unknown/release/a11y_theming_bench.wasm"
  cargo build -q --release --lib --target wasm32-unknown-unknown "${args[@]}" --target-dir "target/wasm-$v"
  f="target/wasm-$v/wasm32-unknown-unknown/release/a11y_theming_bench.wasm"
  echo "$v raw $(stat -f%z "$f") gzip9 $(gzip -9c "$f" | wc -c | tr -d ' ')"
done

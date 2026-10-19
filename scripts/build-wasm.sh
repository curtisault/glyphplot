#!/usr/bin/env bash
# mise run build:wasm: one release module a chart family, built with that
# family's feature alone into web/pkg/<family>/glyphplot{.js,_bg.wasm}, then
# the manifest, the budgets and the sizes beside them (REQ-BUILD-030).
set -euo pipefail
cd "$(dirname "$0")/.."

families=$(cargo run -q -p glyphplot --example manifest -- families)
rm -rf web/pkg
for family in $families; do
  # --out-dir is relative to the crate.
  wasm-pack --quiet build crates/wasm --target web --release --no-pack \
    --out-dir "../../web/pkg/$family" --out-name glyphplot \
    -- --no-default-features --features "$family"
  rm -f "web/pkg/$family/.gitignore"
done
cargo run -q -p glyphplot --example manifest -- write web/pkg
node scripts/size.mjs --write

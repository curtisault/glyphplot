#!/usr/bin/env bash
# mise run build:wasm: one release module a chart family, built with that
# family's feature alone into web/pkg/<family>/glyphplot{.js,_bg.wasm}, then
# the manifest, the budgets and the sizes beside them (REQ-BUILD-030).
#
# mise run build:wasm:debug (this script with --debug): one module, `all`,
# with every family and the `debug` feature (the panic hook and the
# `debug-panic` chart, REQ-WASM-060), built in its own cargo target dir,
# target/wasm-debug, so it never evicts the release build's artefacts; its
# manifest maps every type to `all` and falls back to it (REQ-MANIFEST-020).
# It is built with the release profile, so a panic traps as it does in
# release (REQ-WASM-040).
#
# Either way the modules are built into a staging folder beside web/pkg and
# swapped in whole once everything has built, so a failed build leaves the
# last good web/pkg (REQ-BUILD-040).
set -euo pipefail
cd "$(dirname "$0")/.."
root=$PWD

debug=false
case "${1:-}" in
  "") ;;
  --debug) debug=true ;;
  *) echo "usage: build-wasm.sh [--debug]" >&2; exit 2 ;;
esac

stage=$root/web/.pkg-stage.$$
old=$root/web/.pkg-old.$$
rm -rf "$stage" "$old"
trap 'rm -rf "$stage" "$old"' EXIT
mkdir -p "$stage"

# build <module> <features>: one module into the staging folder.
build() {
  wasm-pack --quiet build crates/wasm --target web --release --no-pack \
    --out-dir "$stage/$1" --out-name glyphplot \
    -- --no-default-features --features "$2"
  rm -f "$stage/$1/.gitignore"
}

if $debug; then
  (export CARGO_TARGET_DIR="$root/target/wasm-debug" && build all "all debug")
  cargo run -q -p glyphplot --example manifest -- write "$stage" --debug
else
  for family in $(cargo run -q -p glyphplot --example manifest -- families); do
    build "$family" "$family"
  done
  cargo run -q -p glyphplot --example manifest -- write "$stage"
fi
node scripts/size.mjs --write --pkg "$stage"

# The swap: the old web/pkg out, the new one in, the old one gone.
if [ -d web/pkg ]; then
  mv web/pkg "$old"
fi
mv "$stage" web/pkg
rm -rf "$old"
echo "build-wasm: web/pkg holds $(cd web/pkg && ls -d */ | tr -d / | tr '\n' ' ')$($debug && echo '(debug)')"

#!/usr/bin/env bash
# mise run dev [-- <vite flags>]: the debug build served with the checklist
# page, rebuilt when a file under crates/ changes, the open pages reloaded
# (REQ-BUILD-080).
#
# - builds the debug wasm (`all`, every family plus `debug`) into web/pkg;
# - watchexec (pinned in mise.toml) reruns that build on a change under
#   crates/ (.rs or .toml), one build at a time, the next queued; a failed
#   build leaves the last good web/pkg (build-wasm.sh swaps it in whole);
# - Vite serves site/ with `glyphplot` aliased to web/src: a change in web/src
#   reloads through Vite, and site/vite.config.ts reloads every page when
#   web/pkg is swapped.
#
# The page is http://localhost:5310/test.html; `mise run dev -- --host`
# serves it to the network, for a phone. A release build (`mise run
# build:wasm`, `mise run test:site`) swaps web/pkg back to release while this
# runs; the pages reload onto it until the next crates/ change.
set -euo pipefail
cd "$(dirname "$0")/.."

echo "dev: building the debug wasm"
bash scripts/build-wasm.sh --debug

watchexec --postpone --watch crates --exts rs,toml --debounce 500ms --on-busy-update queue \
  -- bash scripts/build-wasm.sh --debug &
watcher=$!
trap 'kill "$watcher" 2>/dev/null || true' EXIT INT TERM

echo "dev: the checklist is http://localhost:5310/test.html (the debug build: its panic case runs)"
cd site
npx vite --port 5310 --strictPort "$@"

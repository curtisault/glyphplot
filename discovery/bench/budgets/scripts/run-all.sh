#!/usr/bin/env bash
# Runs every timing in turn (not side by side), each with uptime before and
# after, into results/<name>-<tag>.txt. Run from the glyphplot root after
# ladder.sh and pad.mjs:  mise exec -- discovery/bench/budgets/scripts/run-all.sh <tag>
set -uo pipefail
B=discovery/bench/budgets
tag="${1:-run}"
t() { local name=$1; shift; { date; uptime; "$@"; uptime; } > "$B/results/$name-$tag.txt" 2>&1; }
t native "$B/ladder/target/release/examples/native" 5
t node node "$B/js/node-bench.mjs" bench 5
t chromium node "$B/js/browser-bench.mjs" 5 chromium
PLAYWRIGHT_BROWSERS_PATH=0 t webkit node "$B/js/browser-bench.mjs" 5 webkit
t load node "$B/js/load-bench.mjs" 7

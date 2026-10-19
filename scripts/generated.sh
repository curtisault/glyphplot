#!/usr/bin/env bash
# Generated output stays out of git (REQ-BUILD-060): `mise run lint` fails
# when a tracked file lies under one of the paths the builds write.
set -euo pipefail
cd "$(dirname "$0")/.."
tracked=$(git ls-files -- web/pkg web/dist site/dist target ':(glob)discovery/bench/**/target/**' ':(glob)discovery/bench/**/pkg/**' ':(glob)discovery/bench/**/node_modules/**')
if [ -n "$tracked" ]; then
  echo "generated: generated output is tracked (REQ-BUILD-060):"
  echo "$tracked" | sed 's/^/  /'
  exit 1
fi

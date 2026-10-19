#!/usr/bin/env bash
# The dependency rules `mise run lint` checks:
# - the core's normal dependencies are serde, serde_json and unicode-width at
#   most (REQ-ARCH-010);
# - no git dependencies (REQ-ARCH-070);
# - nothing from unicel's repository (REQ-INDEP-020): no Cargo.toml,
#   package.json, mise.toml or file under scripts/ may name animate-unicode,
#   or a unicel crate or package, outside a comment line. Citing unicel as
#   prior art in a comment is allowed (ADR-013). This file is the one
#   exception, since it names what it looks for.
set -euo pipefail
cd "$(dirname "$0")/.."
status=0

core_deps=$(cargo tree -e normal -p glyphplot --depth 1 --prefix none --format '{p}' | awk 'NR > 1 { print $1 }' | sort -u | grep -vxE 'serde|serde_json|unicode-width' || true)
if [ -n "$core_deps" ]; then
  echo "deps: the core depends on more than serde, serde_json and unicode-width (REQ-ARCH-010):"
  echo "$core_deps" | sed 's/^/  /'
  status=1
fi

git_deps=$(cargo metadata --format-version 1 | grep -o '"source":"git+[^"]*"' | sort -u || true)
if [ -n "$git_deps" ]; then
  echo "deps: git dependencies (REQ-ARCH-070):"
  echo "$git_deps" | sed 's/^/  /'
  status=1
fi

# Tracked and untracked files, without the ignored ones (target/, node_modules/, …).
files=$(git ls-files --cached --others --exclude-standard -- '*Cargo.toml' '*package.json' 'mise.toml' 'scripts/' | sort -u | grep -vxF 'scripts/deps.sh' || true)
for file in $files; do
  [ -f "$file" ] || continue
  hits=$(grep -niE 'animate-unicode|unicel' "$file" | grep -vE '^[0-9]+:[[:space:]]*(#|//)' || true)
  if [ -n "$hits" ]; then
    echo "deps: $file names unicel's repository (REQ-INDEP-020, ADR-013):"
    echo "$hits" | sed 's/^/  line /'
    status=1
  fi
done

exit $status

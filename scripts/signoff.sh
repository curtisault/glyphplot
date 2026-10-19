#!/usr/bin/env bash
# mise run signoff [rev]: the CI, run locally, as there is no hosted CI
# (REQ-CI-010). Checks a commit (HEAD by default; in a GitButler workspace,
# HEAD is every applied branch merged) out into target/ci/tree, a detached
# git worktree removed afterwards, and runs `mise run check` there, so only
# what is committed is checked and git sees that commit's files. The log goes
# to target/ci/logs/<sha>.log.
#
# From v0.4.0 it signs off each open pull request's head and posts a
# `signoff` commit status (REQ-CI-020).
set -euo pipefail
cd "$(dirname "$0")/.."
root=$PWD

rev=${1:-HEAD}
sha=$(git rev-parse --verify "$rev^{commit}")
tree=$root/target/ci/tree
log=$root/target/ci/logs/$sha.log
mkdir -p "$root/target/ci/logs"
git worktree remove --force "$tree" 2>/dev/null || rm -rf "$tree"
git worktree prune
git worktree add --quiet --detach "$tree" "$sha"
trap 'git -C "$root" worktree remove --force "$tree"' EXIT

echo "signoff: $rev ($sha), log in ${log#"$root"/}"
# The tree's own mise.toml, trusted for this run; one cargo cache across runs.
if (cd "$tree" && MISE_TRUSTED_CONFIG_PATHS="$tree" CARGO_TARGET_DIR="$root/target/ci/target" mise run check) >"$log" 2>&1; then
  echo "signoff: $sha passed"
else
  tail -n 40 "$log"
  echo "signoff: $sha failed; the whole log is ${log#"$root"/}"
  exit 1
fi

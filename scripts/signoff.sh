#!/usr/bin/env bash
# mise run signoff: the CI, run locally, as there is no hosted CI (REQ-CI-010,
# REQ-CI-020).
#
#   signoff.sh                 every open pull request (`gh pr list`), oldest first
#   signoff.sh 5 '#6'          those pull requests
#   signoff.sh <rev>           a local revision (HEAD, a branch, a sha): no status posted
#   signoff.sh --list [...]    print the queue and stop
#   signoff.sh --no-status ... check pull requests without posting statuses
#
# Each target's commit is checked out on its own into target/ci/tree, a
# detached git worktree removed afterwards, so only what is committed is
# checked and git sees that commit's files. In a GitButler workspace HEAD is
# the workspace's merge commit (every applied branch merged), not a pull
# request's head, so a pull request is checked at its own head commit as
# GitHub reports it (`headRefOid`), fetched from origin when it is not
# local. A stacked pull request's head contains the ones below it; each is
# checked on its own.
#
# In the tree, with that tree's own mise.toml (mise is stopped from reading
# the repository's, above it, by MISE_CEILING_PATHS), PROPTEST_CASES=4096 and
# GLYPHPLOT_SIGNOFF=1, one cargo target dir shared across runs
# (target/ci/target), these steps run in turn, each logged and timed:
#
#   tools        mise install (a no-op once the pinned tools are in)
#   npm ci       web/ and site/, each that has a package-lock.json
#   check        lint, test (all of test:site under GLYPHPLOT_SIGNOFF), build,
#                size; its time is REQ-BUILD-090's figure
#   test:site    only when the tree's `test` does not already run all of it
#   test:budgets the native budgets (rule 30), when the tree has the task
#   bench:web    frames and pointer moves in headless Chromium
#   bench:load   the first frame on a throttled network
#
# A step the tree's mise.toml does not define is "n/a", not a failure: a
# branch that predates a task cannot run it. A tree with no `check` task
# (no mise.toml at all, as `init`) is skipped and gets no status: GitHub's
# commit statuses have no neutral state, and posting success for a commit
# nothing checked would read as a pass.
#
# Pull requests run one after another, never at once: the benches need a
# quiet machine, and test:site's preview server takes a fixed port. Each
# target's log is target/ci/logs/<key>.log; for a pull request a `signoff`
# commit status (success or failure, a one-line description) is posted on
# its head with `gh api repos/{owner}/{repo}/statuses/<sha>`. A summary
# table ends the run (also in target/ci/summary.txt). The exit is 1 when any
# target failed, after checking the rest; skips are not failures.
set -euo pipefail
cd "$(dirname "$0")/.."
root=$PWD
ci=$root/target/ci
tree=$ci/tree
logs=$ci/logs
mkdir -p "$logs"

usage() {
  sed -n '5,10p' "$0" | sed 's/^# \{0,1\}//' >&2
  exit 2
}

list=false
post=true
args=()
while [ $# -gt 0 ]; do
  case "$1" in
    --list) list=true ;;
    --no-status) post=false ;;
    -h | --help) usage ;;
    -*) usage ;;
    *) args+=("$1") ;;
  esac
  shift
done

# ------------------------------------------------------------------ the queue
# One line a target: kind (pr|rev), key, sha, label.
queue=()
add_pr() {
  local n=$1 json
  json=$(gh pr view "$n" --json number,headRefOid,headRefName,state --jq '"\(.number) \(.headRefOid) \(.headRefName) \(.state)"') || {
    echo "signoff: no pull request #$n" >&2
    exit 2
  }
  read -r num sha branch state <<<"$json"
  [ "$state" = OPEN ] || echo "signoff: #$num is $state; checking it anyway" >&2
  queue+=("pr pr$num $sha #$num $branch")
}
if [ ${#args[@]} -eq 0 ]; then
  command -v gh >/dev/null || {
    echo "signoff: gh is needed to list the open pull requests; give a revision instead" >&2
    exit 2
  }
  while read -r num sha branch; do
    [ -n "$num" ] && queue+=("pr pr$num $sha #$num $branch")
  done < <(gh pr list --state open --limit 200 --json number,headRefOid,headRefName --jq 'sort_by(.number) | .[] | "\(.number) \(.headRefOid) \(.headRefName)"')
  [ ${#queue[@]} -gt 0 ] || {
    echo "signoff: no open pull request"
    exit 0
  }
else
  for a in "${args[@]}"; do
    case "$a" in
      \#[0-9]* | pr[0-9]*) add_pr "${a//[!0-9]/}" ;;
      *)
        if [[ "$a" =~ ^[0-9]{1,5}$ ]]; then
          add_pr "$a"
        else
          sha=$(git rev-parse --verify --quiet "$a^{commit}") || {
            echo "signoff: $a is neither a pull request number nor a revision" >&2
            exit 2
          }
          queue+=("rev ${sha:0:12} $sha $a -")
        fi
        ;;
    esac
  done
fi

if $list; then
  for q in "${queue[@]}"; do
    read -r kind key sha label branch <<<"$q"
    printf '%-4s %-8s %s %s\n' "$kind" "$label" "${sha:0:12}" "$([ "$branch" = - ] || echo "$branch")"
  done
  exit 0
fi

# ------------------------------------------------------------------ one at a time
lock=$ci/lock
if ! mkdir "$lock" 2>/dev/null; then
  other=$(cat "$lock/pid" 2>/dev/null || true)
  if [ -n "$other" ] && kill -0 "$other" 2>/dev/null; then
    echo "signoff: another signoff (pid $other) is running; target/ci is its" >&2
    exit 2
  fi
  rm -rf "$lock" && mkdir "$lock"
fi
echo $$ >"$lock/pid"
cleanup() {
  git -C "$root" worktree remove --force "$tree" 2>/dev/null || rm -rf "$tree"
  git -C "$root" worktree prune
  rm -rf "$lock"
}
trap cleanup EXIT
trap 'exit 130' INT
trap 'exit 143' TERM

dur() { printf '%dm%02ds' $(($1 / 60)) $(($1 % 60)); }
has_task() { grep -Eq "^\[tasks\.(\"$1\"|$1)\]" "$tree/mise.toml" 2>/dev/null; }

# post <sha> <state> <description>: the `signoff` status on a commit.
post_status() {
  local out
  if out=$(gh api -X POST "repos/{owner}/{repo}/statuses/$1" -f state="$2" -f context=signoff -f description="${3:0:140}" --jq .state 2>&1); then
    echo "signoff: posted $2 on ${1:0:12}"
  else
    echo "signoff: could not post the status on ${1:0:12}: $out" >&2
  fi
}

summary=()
failed=0
started_all=$(date +%s)
echo "signoff: ${#queue[@]} target(s), one at a time; $(uptime | sed 's/.*load average[s]*: */load averages /')"

for q in "${queue[@]}"; do
  read -r kind key sha label branch <<<"$q"
  log=$logs/$key.log
  name="$label$([ "$branch" = - ] || echo " $branch")"
  echo
  echo "signoff: $name at ${sha:0:12}, log in ${log#"$root"/}; $(uptime | sed 's/.*load average[s]*: */load averages /')"
  : >"$log"
  t0=$(date +%s)

  if ! git cat-file -e "$sha^{commit}" 2>/dev/null; then
    # Only the objects and FETCH_HEAD are written: no branch or ref moves.
    git fetch --quiet --no-tags origin "$sha" >>"$log" 2>&1 || true
  fi
  if ! git cat-file -e "$sha^{commit}" 2>/dev/null; then
    echo "signoff: ${sha:0:12} is not here and could not be fetched" | tee -a "$log"
    summary+=("$name|${sha:0:7}|FAIL|$(dur 0)|commit not found")
    failed=$((failed + 1))
    [ "$kind" = pr ] && $post && post_status "$sha" failure "signoff: the head commit could not be fetched"
    continue
  fi

  git worktree remove --force "$tree" 2>/dev/null || rm -rf "$tree"
  git worktree prune
  git worktree add --quiet --detach "$tree" "$sha"

  if ! has_task check; then
    why=$([ -f "$tree/mise.toml" ] && echo "its mise.toml has no check task" || echo "no mise.toml at this commit")
    echo "signoff: skipped, $why: nothing to sign off; no status posted" | tee -a "$log"
    summary+=("$name|${sha:0:7}|SKIP|$(dur 0)|$why")
    continue
  fi

  passed=()
  failures=()
  absent=()
  check_time=""
  # step <name> <command>: runs in the tree, logged; records pass or fail.
  step() {
    local s=$1 cmd=$2 a b rc
    a=$(date +%s)
    echo "signoff:   $s ..."
    {
      echo "==== $s: $cmd ($(date '+%F %T'); $(uptime | sed 's/.*load average[s]*: */load averages /'))"
    } >>"$log"
    set +e
    (cd "$tree" && env \
      MISE_TRUSTED_CONFIG_PATHS="$tree" \
      MISE_CEILING_PATHS="$ci" \
      MISE_YES=1 \
      CARGO_TARGET_DIR="$ci/target" \
      PROPTEST_CASES=4096 \
      GLYPHPLOT_SIGNOFF=1 \
      bash -c "$cmd") >>"$log" 2>&1 </dev/null
    rc=$?
    set -e
    b=$(date +%s)
    if [ $rc -eq 0 ]; then
      passed+=("$s $(dur $((b - a)))")
      echo "==== $s: passed in $(dur $((b - a)))" >>"$log"
      echo "signoff:   $s passed in $(dur $((b - a)))"
    else
      failures+=("$s")
      echo "==== $s: FAILED (exit $rc) in $(dur $((b - a)))" >>"$log"
      echo "signoff:   $s FAILED (exit $rc) in $(dur $((b - a)))"
    fi
    [ "$s" = check ] && check_time=$(dur $((b - a)))
    return $rc
  }

  step tools "mise install --quiet" || true
  npm_cmd=""
  for d in web site; do
    [ -f "$tree/$d/package-lock.json" ] && npm_cmd+="npm ci --no-audit --no-fund --prefix $d && "
  done
  if [ -n "$npm_cmd" ]; then
    step "npm ci" "${npm_cmd% && }" || true
  else
    absent+=("npm ci")
  fi
  if [ ${#failures[@]} -eq 0 ]; then
    step check "mise run check" || true
    # The tree's `test` runs all of test:site under GLYPHPLOT_SIGNOFF from v0.3.0; before that, run it here.
    if has_task test:site && ! grep -q GLYPHPLOT_SIGNOFF "$tree/mise.toml"; then
      step test:site "mise run test:site" || true
    elif ! has_task test:site; then
      absent+=("test:site")
    fi
    # The native budgets (contract rule 30); the hook stays until a tree has the task.
    for s in test:budgets bench:web bench:load; do
      if has_task "$s"; then
        step "$s" "mise run $s" || true
      else
        absent+=("$s")
      fi
    done
  fi

  t1=$(date +%s)
  took=$(dur $((t1 - t0)))
  na=""
  [ ${#absent[@]} -gt 0 ] && na="; n/a: $(
    IFS=,
    echo "${absent[*]}" | sed 's/,/, /g'
  )"
  [ -n "$check_time" ] && echo "signoff: check took $check_time (REQ-BUILD-090)" | tee -a "$log"
  if [ ${#failures[@]} -eq 0 ]; then
    detail="check $check_time$na"
    summary+=("$name|${sha:0:7}|PASS|$took|$detail")
    echo "signoff: $name passed in $took"
    [ "$kind" = pr ] && $post && post_status "$sha" success "signoff passed in $took: check $check_time$na"
  else
    # Which of check's tasks failed, from mise's "[task] ERROR" lines.
    sub=$(grep -Eo '^\[[a-z:-]+\] ERROR' "$log" | sed -E 's/^\[([^]]+)\].*/\1/' | sort -u | tr '\n' ' ' | sed 's/ $//')
    what=$(
      IFS=,
      echo "${failures[*]}" | sed 's/,/, /g'
    )
    [ -n "$sub" ] && what="$what ($sub)"
    summary+=("$name|${sha:0:7}|FAIL|$took|failed: $what")
    failed=$((failed + 1))
    echo "signoff: $name FAILED: $what; the whole log is ${log#"$root"/}"
    grep -E 'ERROR|FAILED|error(\[|:)|✘|failed' "$log" | tail -n 15 | sed 's/^/  | /' || true
    [ "$kind" = pr ] && $post && post_status "$sha" failure "signoff failed: $what; log target/ci/logs/$key.log"
  fi
done

# ------------------------------------------------------------------ summary
{
  echo
  echo "signoff: $(date '+%F %T'), $(dur $(($(date +%s) - started_all))) in all; $(uptime | sed 's/.*load average[s]*: */load averages /')"
  printf '%-26s %-8s %-5s %-8s %s\n' target head result time detail
  printf '%-26s %-8s %-5s %-8s %s\n' -------------------------- -------- ----- -------- ------
  for s in "${summary[@]}"; do
    IFS='|' read -r a b c d e <<<"$s"
    printf '%-26s %-8s %-5s %-8s %s\n' "$a" "$b" "$c" "$d" "$e"
  done
} | tee "$ci/summary.txt"
[ $failed -eq 0 ] || exit 1

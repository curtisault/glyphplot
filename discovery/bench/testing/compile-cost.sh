#!/usr/bin/env bash
# Clean builds of the test targets with no property-test crate, with proptest
# and with quickcheck, in the dev and the release (LTO) profile, three times
# each, in their own target dirs. Prints seconds a build, with uptime's load.
set -euo pipefail
cd "$(dirname "$0")"
for profile in dev release; do
  for feat in none proptest quickcheck; do
    for run in 1 2 3; do
      dir=target/cc-$profile-$feat
      rm -rf "$dir"
      flags=(--tests); [ "$profile" = release ] && flags+=(--release); [ "$feat" != none ] && flags+=(--features "$feat")
      load=$(uptime | sed 's/.*averages*: //')
      start=$(python3 -c "import time;print(time.time())")
      CARGO_TARGET_DIR=$dir cargo build -q "${flags[@]}"
      end=$(python3 -c 'import time;print(time.time())')
      printf '%s %s run %s: %.1f s (load %s)\n' "$profile" "$feat" "$run" "$(python3 -c "print($end - float('$start'))")" "$load"
    done
  done
done

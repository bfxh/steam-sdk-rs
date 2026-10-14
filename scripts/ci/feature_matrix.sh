#!/usr/bin/env bash
# Each optional feature has to build on its own. `--all-features` hides a feature whose own
# `cfg` block stopped compiling, and a broken `serde` build is exactly what a user who only
# turned on `serde` hits first.
#
# cargo's own exit status is what decides, not a pipeline's tail -- an earlier draft piped
# through `tail` and so read tail's status, which is always 0.
set -uo pipefail
status=0
for feature in raw-bindings serde image futures; do
  log=$(mktemp)
  echo "--- --no-default-features --features $feature"
  if cargo check --locked --no-default-features --features "$feature" > "$log" 2>&1; then
    tail -2 "$log"
  else
    tail -25 "$log"
    echo "::error::feature '$feature' does not build on its own"
    status=1
  fi
  rm -f "$log"
done
log=$(mktemp)
if cargo check --locked --no-default-features > "$log" 2>&1; then
  echo "--- --no-default-features (no feature at all): ok"
else
  tail -25 "$log"
  echo "::error::the feature-free build does not compile"
  status=1
fi
rm -f "$log"
[ "$status" -eq 0 ] && echo "every optional feature builds alone"
exit $status

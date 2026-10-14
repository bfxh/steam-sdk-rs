#!/usr/bin/env bash
# A `cargo test` run whose filter matches nothing still exits 0 and prints
# "test result: ok. 0 passed", so a green job can mean "not one test ran".
# This gate reads the counts out of the real run and refuses a vacuous one.
set -uo pipefail
suite="${1:?usage: test_floor.sh <lib|doc>}"
floor_file="scripts/ci/baselines/test_floor_${suite}.txt"
# the baseline file carries a trailing comment; only the first field is the number
floor=$(awk '{print $1; exit}' "$floor_file")
out=$(mktemp)

case "$suite" in
  lib) args=(test --locked --lib --all-features) ;;
  doc) args=(test --locked --doc --all-features) ;;
  *) echo "unknown suite: $suite" >&2; exit 2 ;;
esac

status=0
cargo "${args[@]}" > "$out" 2>&1 || status=$?

# awk, not bc: no extra package has to be present on the runner for the count to work.
ran=$(grep -oE '^running [0-9]+ test' "$out" | grep -oE '[0-9]+' | awk '{ s += $1 } END { print s + 0 }')
failed=$(grep -oE '[0-9]+ failed' "$out" | grep -oE '[0-9]+' | awk '{ s += $1 } END { print s + 0 }')
cat "$out"
rm -f "$out"

echo "suite=$suite: $ran tests reached, $failed failed, floor is $floor"
if [ "$ran" -eq 0 ]; then
  echo "::error::no test ran at all -- this job proves nothing"
  exit 1
fi
if [ "$failed" -ne 0 ] || [ "$status" -ne 0 ]; then
  echo "::error::tests failed (exit $status, $failed failed)"
  exit 1
fi
if [ "$ran" -lt "$floor" ]; then
  echo "::error::only $ran tests ran, expected at least $floor -- tests do not disappear on their own"
  exit 1
fi
echo "test floor holds"

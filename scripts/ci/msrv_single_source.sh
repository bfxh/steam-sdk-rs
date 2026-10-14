#!/usr/bin/env bash
# MSRV is declared in more than one place: Cargo.toml, steamworks-sys/Cargo.toml and the
# CI matrix. If they disagree, the job labelled "1.80.0" is testing something other than
# the version the crates promise, and a dependency that needs more will pass CI and break
# for users. Every declaration must equal the value CI actually pins.
set -uo pipefail
expected="${1:?usage: msrv_single_source.sh <msrv-the-ci-matrix-runs>}"
status=0
found=0
for manifest in Cargo.toml steamworks-sys/Cargo.toml; do
  declared=$(sed -n 's/^rust-version = "\([^"]*\)".*/\1/p' "$manifest" | head -1)
  if [ -z "$declared" ]; then
    echo "::error::$manifest declares no rust-version"
    status=1
    continue
  fi
  found=$((found + 1))
  if [ "$declared" != "$expected" ]; then
    echo "::error::$manifest says $declared but CI's MSRV job runs $expected"
    status=1
  else
    echo "$manifest: rust-version $declared == CI MSRV"
  fi
done
if [ "$found" -eq 0 ]; then
  echo "::error::no manifest declared an MSRV at all -- the ruler found nothing"
  exit 1
fi
exit $status

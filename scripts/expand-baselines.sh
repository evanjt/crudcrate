#!/usr/bin/env bash
# Expand the widest downstream surfaces and record the public API of crudcrate.
# Run before and after a refactor; diff the two output directories.
#   scripts/expand-baselines.sh snapshots/before
#   ...make changes...
#   scripts/expand-baselines.sh snapshots/after
#   diff -r snapshots/before snapshots/after
# Expanded files contain absolute paths (from tracing call sites), so they are
# gitignored. Requires cargo-expand; the public API listing requires
# cargo-public-api and a nightly toolchain.
set -euo pipefail
out=${1:-snapshots}
mkdir -p "$out"
rustc --version > "$out/rustc-version.txt"
for ex in minimal recursive_join scoped_access joined_filter; do
  cargo expand -p crudcrate --example "$ex" --features derive,sqlite > "$out/example_$ex.rs"
done
cargo expand -p test_suite --test integer_pk_test > "$out/test_integer_pk.rs"
if cargo public-api --version >/dev/null 2>&1; then
  cargo +nightly public-api -p crudcrate --simplified > "$out/public-api.txt"
fi

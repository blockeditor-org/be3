#!/bin/bash
# Runs COUNT builds at once on this VM: each in its own worktree of ~/be3 with its own buck2 daemon, each with a
# different one-file change to be-lib-geometry, so none comes from the cache and they compete for the workers.
# Usage: concurrent-builds.sh NL_URL COUNT SEED     (~/nl-be3, or $BE3_DIR, is a checkout whose target/tools has been built)
set -uo pipefail
url=${1:?BE3_NL_URL}; count=${2:?count}; seed=${3:-1}
export BE3_BUILD_SERVER=nativelink BE3_NL_URL="$url" BE3_BOOTSTRAP=cargo
base=${BE3_DIR:-$HOME/nl-be3}
cd "$base"
for i in $(seq 1 "$count"); do
  dir="$HOME/nl-be3-c$seed-$i"
  [ -d "$dir" ] || { git worktree add -q --detach "$dir" HEAD; mkdir -p "$dir/target"; cp -a target/tools "$dir/target/"; }
  printf '\npub const BENCH_N: u32 = %d;\n' $((seed * 100000 + i * 1000 + RANDOM % 1000)) >> "$dir/crates/be-lib-geometry/src/lib.rs"
done
# Not timed: the first command in a worktree builds its bootstrap and starts its daemon and relay.
for i in $(seq 1 "$count"); do
  ( cd "$HOME/nl-be3-c$seed-$i" && ./scripts/buck targets //crates/be-tool-run-test: > /dev/null 2>&1 ) &
done
wait
echo "start $(date +%T)"
for i in $(seq 1 "$count"); do
  ( cd "$HOME/nl-be3-c$seed-$i" && s=$(date +%s.%N) && ./scripts/verify > "/tmp/verify-c$seed-$i.log" 2>&1; rc=$?
    echo "build $seed-$i exit $rc in $(echo "$(date +%s.%N) - $s" | bc) s" ) &
done
wait
echo "end $(date +%T)"

#!/bin/bash
# Times what a new client VM does before a fully warm build (every action already in the scheduler's cache): a partial
# clone, the cargo-built bootstrap, the first and second command, then `verify` twice. Run it on the VM once its route to
# the build server works. Usage: fresh-client.sh BRANCH NL_URL
set -uo pipefail
branch=${1:?branch}; url=${2:?BE3_NL_URL}
# A new boat VM already has a ~/be3 of the environment's; this clones a checkout of its own.
dir=${BE3_DIR:-$HOME/nl-be3}
now() { date +%s.%N; }
took() { printf '%-52s %7.1f s\n' "$1" "$(echo "$(now) - $2" | bc)"; }
export PATH="$HOME/.cargo/bin:/usr/local/cargo/bin:$PATH"
t=$(now); git -c credential.helper='!gh auth git-credential' clone -q --filter=blob:none --branch "$branch" \
  https://github.com/blockeditor-org/be3.git "$dir"; took "git clone, partial" "$t"
cd "$dir"
t=$(now); cargo build -q --locked --release -p be-tool-bootstrap --target-dir target/tools/bootstrap-cargo 2>/dev/null
took "the bootstrap built with cargo (until a release is pinned)" "$t"
export BE3_BUILD_SERVER=nativelink BE3_NL_URL="$url" BE3_BOOTSTRAP=cargo
t=$(now); ./scripts/buck targets //crates/be-tool-run-test: > /tmp/first.log 2>&1; took "first command: buck2 download, relay build, daemon" "$t"
t=$(now); ./scripts/buck targets //crates/be-tool-run-test: > /dev/null 2>&1; took "second command, nothing new to start" "$t"
t=$(now); ./scripts/verify > /tmp/verify-fresh.log 2>&1; rc=$?; took "verify, fully warm cache, new daemon (exit $rc)" "$t"
grep -E "Commands:|Cache hits" /tmp/verify-fresh.log | tail -2
t=$(now); ./scripts/verify > /tmp/verify-fresh2.log 2>&1; took "verify again, daemon warm as well" "$t"

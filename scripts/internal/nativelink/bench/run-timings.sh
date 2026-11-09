#!/bin/bash
# Takes the two timings in guides/build_server.md: a new client VM running a fully warm build, and several builds at once.
# It makes CLIENTS client VMs, which reach the build server as any client does: through boat's public edge, at NL_URL
# (BE3_NL_URL, token included).
# Needs a `boat` login that may create sandboxes and ssh to the scheduler, and a scheduler that is already warm for BRANCH.
#   SCHED=bx_... NL_URL=https://...?_token=... BRANCH=nativelink-build-server [BOAT_HOME=DIR] bash run-timings.sh [CLIENTS=3] [BUILDS=8]
set -uo pipefail
: "${SCHED:?scheduler sandbox id}" "${NL_URL:?BE3_NL_URL}" "${BRANCH:?branch}"
CLIENTS=${1:-3}; BUILDS=${2:-8}
HERE=$(dirname "$(readlink -f "$0")")
boat() { env ${BOAT_HOME:+HOME=$BOAT_HOME} BOAT_NO_UPDATE=1 boat "$@"; }
URL=$NL_URL
ids=()
say() { printf '%s %s\n' "$(date +%T)" "$*"; }
starts_left() { boat --json limits | python3 -c 'import sys,json; d=json.load(sys.stdin)["starts"]; print(d["day"]["remaining"])'; }

new_client() {  # NUMBER: a new VM
  boat new --name "nl-bench-$1" --ttl 7200 --no-snapshots 2>&1 | python3 -c 'import sys,json
for l in sys.stdin:
    try: d=json.loads(l)
    except ValueError: continue
    if d.get("event")=="ready": print(d["id"]); break'
}

say "starts left today: $(starts_left)"
for i in $(seq 1 "$CLIENTS"); do
  ( new_client "$i" > "/tmp/nl-bench-$i.id" ) &
done; wait
for i in $(seq 1 "$CLIENTS"); do ids+=("$(cat /tmp/nl-bench-$i.id)"); say "client $i: ${ids[$((i-1))]}"; done

say "=== a new client VM, fresh checkout and daemon, fully warm build"
boat ssh "${ids[0]}" 'cat > /tmp/fresh-client.sh' < "$HERE/fresh-client.sh"
boat ssh "${ids[0]}" "bash /tmp/fresh-client.sh $BRANCH $URL" 2>&1 | tee /tmp/nl-bench-fresh.txt

say "=== the other clients get a checkout and a started daemon (not timed)"
for i in $(seq 2 "$CLIENTS"); do
  ( boat ssh "${ids[$((i-1))]}" "bash -s $BRANCH $URL" > /tmp/nl-bench-prep-$i.txt 2>&1 <<'PREP'
export PATH="$HOME/.cargo/bin:/usr/local/cargo/bin:$PATH"
git -c credential.helper='!gh auth git-credential' clone -q --filter=blob:none --branch "$1" https://github.com/blockeditor-org/be3.git nl-be3 && cd nl-be3 || exit 1
cargo build -q --locked --release -p be-tool-bootstrap --target-dir target/tools/bootstrap-cargo 2>/dev/null
BE3_BUILD_SERVER=nativelink BE3_NL_URL="$2" BE3_BOOTSTRAP=cargo ./scripts/buck targets //crates/be-tool-run-test: > /dev/null 2>&1; echo prepared
PREP
  ) &
done; wait

say "=== $BUILDS builds at once, split over $CLIENTS clients"
before=$(starts_left); since=$(date -u +'%Y-%m-%d %H:%M:%S')
per=$(( (BUILDS + CLIENTS - 1) / CLIENTS )); left=$BUILDS
for i in $(seq 1 "$CLIENTS"); do
  n=$(( left < per ? left : per )); left=$(( left - n )); [ "$n" -gt 0 ] || continue
  ( boat ssh "${ids[$((i-1))]}" 'cat > /tmp/concurrent-builds.sh' < "$HERE/concurrent-builds.sh"
    boat ssh "${ids[$((i-1))]}" "bash /tmp/concurrent-builds.sh $URL $n $i" 2>&1 | sed "s/^/[client $i] /" ) &
done | tee /tmp/nl-bench-concurrent.txt; wait
say "starts used by the burst and the clients: $(( before - $(starts_left) ))"
echo "--- what the controller did:"; boat ssh "$SCHED" "sudo journalctl -u nl-controller --since '$since' --no-pager | sed 's/^.*python3\[[0-9]*\]: //' | grep -E 'scaling|target|kept|created|start budget' | cut -c1-150" 2>/dev/null | tail -40

say "=== cleaning up"
for i in $(seq 1 "$CLIENTS"); do
  [ -n "${ids[$((i-1))]}" ] && boat delete --yes "${ids[$((i-1))]}" > /dev/null 2>&1
done

#!/bin/bash
# Makes a scheduler VM in the nl-scheduler environment and sets it up. Usage:
#   new-scheduler.sh NAME BOAT_WORKERS_KEY_FILE [TYPE]      (TYPE defaults to small)
# Run it where `boat` is logged in with a key that may create sandboxes in the nl-scheduler environment and reach them.
# The environments, their secret files and the keys have to exist first: guides/build_server.md, "Starting over".
set -euo pipefail
name=${1:?usage: new-scheduler.sh NAME BOAT_WORKERS_KEY_FILE [TYPE]}; key=${2:?boat key file}; type=${3:-small}
here=$(dirname "$(readlink -f "$0")")
export BOAT_NO_UPDATE=1
id=$(boat new --name "$name" --environment nl-scheduler --type "$type" --ttl 28800 2>&1 \
     | python3 -c 'import sys,json
for l in sys.stdin:
    try: d=json.loads(l)
    except ValueError: continue
    if d.get("event")=="created": print(d["id"]); break')
[ -n "$id" ] || { echo "could not create the sandbox" >&2; exit 1; }
echo "scheduler VM $id"
for _ in $(seq 1 60); do
  [ "$(boat --json info "$id" | python3 -c 'import sys,json; d=json.load(sys.stdin); print(d.get("sandbox",d)["state"])')" = idle ] && break; sleep 3
done
tar czf - -C "$here" scheduler worker | boat ssh "$id" 'rm -rf /tmp/nl && mkdir -p /tmp/nl && tar xzf - -C /tmp/nl'
boat ssh "$id" 'sudo bash /tmp/nl/scheduler/setup-scheduler.sh'
boat ssh "$id" 'cat > /tmp/boat-workers.key' < "$key"
boat ssh "$id" 'sudo bash /tmp/nl/scheduler/install-controller.sh /tmp/boat-workers.key; shred -u /tmp/boat-workers.key'
cat <<NEXT
Done. Left to do by hand:
  1. Expose the tunnel:   boat ssh $id 'host 8099 --localhost; host url 8099'
     and store that address (token included) as the secret BE3_NL_URL.
  2. If the scheduler's address changed, check BE3_NL_KEY still names it (its prefix is nl-sched).
NEXT

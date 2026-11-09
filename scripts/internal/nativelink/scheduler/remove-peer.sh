#!/bin/bash
# On the scheduler. Usage: sudo remove-peer.sh NAME   -- removes the peer from the interface, the registry and, if present, wg-sch.conf
set -euo pipefail
exec 9>/var/lock/wg-sch.lock; flock 9
name=$1
pub=$(awk -v n="$name" '$1==n{print $2}' /run/nl-peers 2>/dev/null | tail -1 || true)
if [ -n "$pub" ]; then
  wg set wg-sch peer "$pub" remove
  grep -v "^$name " /run/nl-peers > /run/nl-peers.new || true; mv /run/nl-peers.new /run/nl-peers
fi
python3 - "$name" <<'PY'
import re,sys
name=sys.argv[1]; p='/etc/wireguard/wg-sch.conf'; s=open(p).read()
m=re.search(r'\n# %s\n\[Peer\]\nPublicKey = (\S+)\nAllowedIPs = [^\n]*\n'%re.escape(name), s)
if m: open(p,'w').write(s.replace(m.group(0),'\n',1))
PY

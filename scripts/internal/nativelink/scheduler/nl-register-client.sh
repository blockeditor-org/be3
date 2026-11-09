#!/bin/bash
# On the scheduler. Usage: sudo nl-register-client.sh c-NAME CLIENT_WIREGUARD_PUBLIC_KEY
# A build client (CI, an agent's VM) calls this over `boat exec` before its build. It keeps its private key; only the
# public key arrives here. The name picks the address, so registering again keeps it and replaces the key.
# Prints key=value lines for the client to write its own WireGuard config from:
#   ip=10.100.0.N   hub_pubkey=...   endpoint=[ipv6]:51822
# The lease lives in the running interface and /run/nl-peers, so it ends with the next restore; the controller also
# drops a client that has not handshaken for hours.
set -euo pipefail
exec 9>/var/lock/wg-sch.lock; flock 9
name=${1:?name}; pub=${2:?public key}
[[ "$name" =~ ^c-[A-Za-z0-9._-]{1,40}$ ]] || { echo "the name must look like c-HOSTNAME" >&2; exit 2; }
[[ "$pub" =~ ^[A-Za-z0-9+/]{43}=$ ]] || { echo "not a WireGuard public key" >&2; exit 2; }
touch /run/nl-peers; chmod 600 /run/nl-peers
ip=$(awk -v n="$name" '$1==n{print $3}' /run/nl-peers | tail -1)
old=$(awk -v n="$name" '$1==n{print $2}' /run/nl-peers | tail -1)
if [ -n "$old" ]; then
  wg set wg-sch peer "$old" remove 2>/dev/null || true
  grep -v "^$name " /run/nl-peers > /run/nl-peers.new || true; mv /run/nl-peers.new /run/nl-peers
fi
if [ -z "$ip" ]; then
  used=$( { awk '{print $3}' /run/nl-peers; grep -oE 'AllowedIPs = [0-9.]+' /etc/wireguard/wg-sch.conf | awk '{print $3}'; } | sort -u)
  for n in $(seq 20 49); do c=10.100.0.$n; grep -qx "$c" <<<"$used" || { ip=$c; break; }; done
fi
[ -n "$ip" ] || { echo "no free client address" >&2; exit 3; }
wg set wg-sch peer "$pub" allowed-ips "$ip/32"
printf '%s %s %s %s\n' "$name" "$pub" "$ip" "$(date +%s)" >> /run/nl-peers
hub=$(ip -6 -o addr show scope global | awk '{print $4}' | cut -d/ -f1 | grep -v '^fd' | head -1)
echo "ip=$ip"
echo "hub_pubkey=$(cat /etc/wireguard/wg-sch.pub)"
echo "endpoint=[$hub]:51822"

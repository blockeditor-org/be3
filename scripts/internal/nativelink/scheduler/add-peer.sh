#!/bin/bash
# On the scheduler. Usage: sudo add-peer.sh NAME 10.100.0.N HUB_ENDPOINT_IPV6 [--persist]
# Adds NAME as a WireGuard spoke and prints the spoke's own config on stdout.
# Workers are ephemeral, so by default the peer lives only in the running interface and in the registry /run/nl-peers
# (both gone after a restore, like the workers). A client's key is static: pass --persist to also write it to wg-sch.conf.
set -euo pipefail
exec 9>/var/lock/wg-sch.lock; flock 9   # the controller adds and removes peers concurrently
name=$1; ip=$2; hub=$3; persist=${4:-}
IF=wg-sch; PORT=51822
priv=$(wg genkey); pub=$(echo "$priv" | wg pubkey); hubpub=$(cat /etc/wireguard/$IF.pub)
wg set $IF peer "$pub" allowed-ips "$ip/32"
printf '%s %s %s\n' "$name" "$pub" "$ip" >> /run/nl-peers
if [ "$persist" = --persist ]; then
  printf '\n# %s\n[Peer]\nPublicKey = %s\nAllowedIPs = %s/32\n' "$name" "$pub" "$ip" >> /etc/wireguard/$IF.conf
fi
cat <<CONF
[Interface]
Address = $ip/24
PrivateKey = $priv

[Peer]
PublicKey = $hubpub
Endpoint = [$hub]:$PORT
AllowedIPs = 10.100.0.0/24
PersistentKeepalive = 25
CONF

#!/bin/bash
# Run before NativeLink starts: creates the /tmp tiers, and when the small tier is empty (a fresh start, or /tmp was
# wiped by a restore) unpacks it from Bunny. It then marks the tier as safe to pack (/run/nl-small-ready), which
# nl-small-pack.sh insists on: an upload made after a restore that failed would replace the good pack with nothing.
# Last, it writes the server's epoch (below).
set -uo pipefail
. /etc/nativelink/server.env
ROOT=/tmp/nativelink
install -d -o nativelink -g nativelink -m 0750 $ROOT $ROOT/cas $ROOT/small $ROOT/ac
ready() { touch /run/nl-small-ready; }
if [ -n "$(find $ROOT/small/content $ROOT/ac/content -type f -print -quit 2>/dev/null)" ]; then
  ready
else
  URL=https://de-s3.storage.bunnycdn.com/be-nativelink-s3/small/pack.tar.zst
  code=$(curl -sS --retry 3 -o $ROOT/pack.restore -w '%{http_code}' --aws-sigv4 "aws:amz:de:s3" \
         --user "$BUNNY_S3_KEY_ID:$BUNNY_S3_SECRET" "$URL" 2>/dev/null || echo 000)
  case "$code" in
    200) if zstd -dc $ROOT/pack.restore | tar -C $ROOT -xf -; then
           logger -t nl-small-restore "restored the small tier from Bunny"; ready
         else
           logger -t nl-small-restore "the pack could not be unpacked; the pack job stays off"
         fi ;;
    404) logger -t nl-small-restore "no pack in Bunny yet; starting with an empty small tier"; ready ;;
    *)   logger -t nl-small-restore "could not read the pack from Bunny (HTTP $code); the pack job stays off" ;;
  esac
  rm -f $ROOT/pack.restore
fi
chown -R nativelink:nativelink $ROOT
# A new epoch for every start: the small tier may have gone back to its last pack, so a client that saw another epoch
# forgets what it took the server to hold (its buck2 daemon and relay restart). Caddy serves it at /nl-epoch.
od -An -N8 -tx1 /dev/urandom | tr -d ' \n' > /run/nl-epoch.new && chmod 644 /run/nl-epoch.new && mv /run/nl-epoch.new /run/nl-epoch

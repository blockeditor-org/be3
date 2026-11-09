#!/bin/bash
# Run before NativeLink starts: creates the /tmp tiers, and when the small tier is
# empty (a fresh start, or /tmp was wiped by a snapshot restore) unpacks it from Bunny.
set -uo pipefail
. /etc/nativelink/server.env
ROOT=/tmp/nativelink
install -d -o nativelink -g nativelink -m 0750 $ROOT $ROOT/cas $ROOT/small $ROOT/ac
if [ -z "$(find $ROOT/small/content $ROOT/ac/content -type f -print -quit 2>/dev/null)" ]; then
  URL=https://de-s3.storage.bunnycdn.com/be-nativelink-s3/small/pack.tar.zst
  if curl -fsS --retry 3 -o $ROOT/pack.restore --aws-sigv4 "aws:amz:de:s3" --user "$BUNNY_S3_KEY_ID:$BUNNY_S3_SECRET" "$URL"; then
    zstd -dc $ROOT/pack.restore | tar -C $ROOT -xf - && logger -t nl-small-restore "restored small tier from Bunny"
  else
    logger -t nl-small-restore "no pack in Bunny; starting with an empty small tier"
  fi
  rm -f $ROOT/pack.restore
fi
chown -R nativelink:nativelink $ROOT

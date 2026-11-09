#!/bin/bash
# Packs the small-object CAS tier and the action cache into ONE object in Bunny.
# Boat snapshots do not save /tmp, and keeping hundreds of thousands of tiny files in
# Bunny (or on the snapshotted disk) is the problem this avoids.
set -euo pipefail
. /etc/nativelink/server.env
[ -e /run/nl-small-ready ] || { logger -t nl-small-pack "not packing: the small tier was not restored from Bunny"; exit 0; }
ROOT=/tmp/nativelink; KEY=small/pack.tar.zst
URL=https://de-s3.storage.bunnycdn.com/be-nativelink-s3/$KEY
tmp=$(mktemp -p $ROOT pack.XXXXXX)
trap 'rm -f "$tmp"' EXIT
# content-addressed files are immutable and renamed into place, so a live tar is safe;
# exit 1 ("file changed/removed") is fine.
tar -C $ROOT --exclude='*/tmp/*' -cf - small/content small/content.exec ac/content ac/content.exec 2>/dev/null \
  | zstd -3 -T2 -q -o "$tmp" -f || [ ${PIPESTATUS[0]} -le 1 ]
size=$(stat -c %s "$tmp")
curl -fsS -o /dev/null -X PUT -T "$tmp" --aws-sigv4 "aws:amz:de:s3" --user "$BUNNY_S3_KEY_ID:$BUNNY_S3_SECRET" \
  -H "x-amz-content-sha256: UNSIGNED-PAYLOAD" "$URL"
logger -t nl-small-pack "uploaded $size bytes to $KEY"

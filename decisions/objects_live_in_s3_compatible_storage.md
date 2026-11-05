# Objects live in S3-compatible storage; only the S3 API is targeted

be-server's object bytes (content-addressed, end-to-end encrypted chunks) can live in a bucket
of any S3-compatible store instead of on the server's disk; the database stays on the server.
The server speaks only the S3 API, and only the part every such store has: path-style URLs,
single-request PUT, GET, HEAD, DELETE and ListObjectsV2, with no versioning, batch delete or
multipart upload. Every request is a SigV4 presigned URL (`rusty-s3`, sans-IO) sent with
`ureq`; a PUT signs its `Content-Length` and `x-amz-checksum-sha256`, so the store refuses any
bytes but the object's. The deployment uses Bunny Storage through its S3 API. With no bucket
configured, objects stay in a `FileStore` beside the database, and the server needs nothing
else to run (be-host embeds it, and every test starts one).

## Why

Disk on the server is the scarce, hard-to-grow part of hosting; object storage is cheap and
grows by itself. The S3 API is the one every provider and self-hosted store speaks, so the
server is not tied to a provider, and tests run a real S3 server (`s3s` over `s3s-fs`) in
process.

## Rejected

- **Bunny Storage's own HTTP API** (`AccessKey` header, as backups use): one provider only, no
  presigned URLs to hand clients, and no checksum the store enforces on upload.
- **Running Garage, RustFS or MinIO beside the server**: another service to deploy, upgrade
  and back up on the same disk the move is meant to relieve.
- **Writing our own S3 server**: the server already answers its clients; storage is what it
  should hand off, and `s3s` covers serving the S3 API where it is needed.

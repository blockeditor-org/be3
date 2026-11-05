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

Clients move bulk object bytes the same way, with one method whether or not a bucket is
configured: the server checks access and quota and answers presigned URLs (`upload-urls`,
`download-urls`), signed against the bucket, or with none against its own S3 endpoint (`s3s`
over its `FileStore`), and the client GETs and PUTs the bytes itself. Only small objects travel
inside protocol frames (a publish's carried objects, `read-contents`). Since bytes arrive after
the reservation, the server confirms an object is in the store (a HEAD) the first time it is
used and records it, and a URL expires well before its reservation can be collected.

## Why

Disk on the server is the scarce, hard-to-grow part of hosting; object storage is cheap and
grows by itself. The S3 API is the one every provider and self-hosted store speaks, so the
server is not tied to a provider, and tests run a real S3 server (`s3s` over `s3s-fs`) in
process.

With a bucket, object bytes then never pass through the server at all: its bandwidth and
disk serve only the protocol, and the store serves the bytes. Having
the server answer the same S3 requests when it keeps the bytes itself means clients have one
way to move them, tested the same way in both cases.

## Rejected

- **Bunny Storage's own HTTP API** (`AccessKey` header, as backups use): one provider only, no
  presigned URLs to hand clients, and no checksum the store enforces on upload.
- **Running Garage, RustFS or MinIO beside the server**: another service to deploy, upgrade
  and back up on the same disk the move is meant to relieve.
- **Writing our own S3 server**: the server already answers its clients; storage is what it
  should hand off, and `s3s` covers serving the S3 API where it is needed.
- **Proxying bytes through the server** (the former `put-objects` and `get-object` calls
  carrying them in frames): every byte crosses the server twice, and a bucket saves it no
  bandwidth.
- **Trusting a reservation as the object's arrival**: an upload URL proves nothing until the
  bytes are in the store, and sending bytes the store already holds proves nothing at all, so
  a reservation of an object already stored does not let its maker hold it.

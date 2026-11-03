# The client-server protocol is not versioned

The protocol between peers and be-server (`be3:server`, postcard over
be-wasm-ops frames) carries no version. A connection starts with a greeting
carrying each side's build id, and the server closes the connection when the
ids differ (`ClientError::Incompatible`); a program with no build id is an
error, not a wildcard.

## Why

Server and clients are upgraded together from one commit, so there is no mix
of protocol versions to support; refusing a mismatched build is enough.

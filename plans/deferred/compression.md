# Compression

Depends on: plans/unified_serialization.md, step 2 (saved data in the new format), for the
stored half; the connection half depends on none.

Set aside until the user picks it up.

plans/unified_serialization.md makes the format fast rather than small: fixed-width
little-endian numbers, so a field or a packed list decodes as a copy. Compression is where size
is won back, as a layer of its own, outside the format, so the compressor can change without
the format changing.

## Where

- **Stored parts**: each block-store part compressed before it is sealed (sealed bytes do not
  compress). Parts are 16–64 KiB, small for a compressor alone; a zstd dictionary trained on
  typical parts, versioned as data, makes up much of that.
- **The server connection**: one streaming compressor per connection, keeping its context
  across messages, since a single live edit or presence update is too small to compress alone.
- **Not host↔guest frames or other local IPC**, where a copy is cheaper than compressing.

## Rules

- A part's hash is of its uncompressed canonical encoding, never of compressed bytes, so its
  identity does not depend on the compressor's version or level.
- Sealing is deterministic (decisions/deterministic_seal_nonce.md), so the same content sealed
  after compressing differently is different bytes, and the server keeps both. Pin the
  compressor and level for stored parts, or accept duplicates after a change of either.
- Measure first, on real documents and op streams: fixed-width numbers against varints, both
  compressed with zstd (and LZ4, which has no entropy coding, so gains less on skewed bytes).
  What no compressor finds is arithmetic: sorted ids, offsets and times shrink most by delta
  encoding, which could be a per-field option of the format (`#[schema(delta)]`) if they show
  up.

## Sizes that leak

Compressing before sealing makes a sealed part's size depend on how well its content
compresses (the CRIME and BREACH attacks). An attacker who can put chosen text into the same
compressed unit as a secret, and see the sealed size each time, can confirm guesses about the
secret a piece at a time.

Anyone who can read the block gains nothing: a compressed unit lies within one part, a part is
sealed under one key, and whoever holds the key already reads all of it. A viewer who changes a
block only through a service that writes into it for them is in that position too. What matters
is a party without the key that sees sealed sizes (the server operator, or the network, which
sees the connection's) and also has a way to put text into a block holding secrets:

- a service or agent that writes outside input (submitted forms, fetched web pages, mail
  received for someone) into a block beside things the sender should not learn;
- the connection's stream compressor, if one stream carries both such input and secrets.

Each guess costs a fresh seal the attacker sees, so the attack is slow, but such a service
makes it automatic. Before compressing either, decide among: rounding sealed sizes up to
buckets (as padding, cheap); keeping outside input in parts or blocks of its own; or not
compressing what a service writes for others. The connection's compressor can reset its
context between messages from different senders.

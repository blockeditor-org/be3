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
secret: a guess that matches the secret compresses better.

Anyone who can read the block gains nothing: a compressed unit lies within one part, a part is
sealed under one key, and whoever holds the key already reads all of it. What matters is
someone who can put text into a block without reading it, together with someone who sees sealed
sizes without holding the key.

**Hosted blocks are that shape** (guides/blocks/hosted.md). A Join member holds no key, but acts
by asking the host, which writes the effects into the block it seals. The member alone sees no
sealed sizes (no objects, no journal), but be-server sees every one, and a share link grants
Join, so an operator can join, or work with someone who did. An exam that hides its answers:
a student's answer is stored in the part beside the right one, and the part shrinking when it
matches says the guess was right. Answers, cards and poll results have few possible values, so
few guesses are needed. A service that writes outside input (submitted forms, fetched pages)
beside secrets into an ordinary block is the same shape.

Rounding sealed sizes up to buckets does not stop it: an attacker who chooses the input's length
pads it until the part sits at a bucket's edge, where a byte saved changes the bucket. So:

- **Nothing that holds input from someone who cannot read it is compressed with what they
  cannot read.** Hosted blocks' content is not compressed at all, in parts or in the journal,
  and a type cannot opt back in; they are small (games, exams, polls, bot chats), so little is
  lost. A block type that writes outside input keeps it in parts or blocks of its own, or is
  not compressed either.
- **A connection's stream compressor shares no context across a hosted block's traffic**, nor
  across messages of different blocks or senders.

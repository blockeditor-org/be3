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

## Sizes that leak, set aside

Every block is compressed, hosted ones included; nothing is built against the following, which
the user judged theoretical: it needs a malicious server, which could more easily serve a
client that reads everything.

Compressing before sealing makes a sealed part's size depend on how well its content compresses
(the CRIME and BREACH attacks): someone who can put chosen text into the same compressed unit as
a secret, and see the sealed size each time, can confirm guesses about the secret. Whoever holds
a block's key gains nothing from it, but a Join member of a hosted block (guides/blocks/hosted.md)
puts text into a block it cannot read, and be-server sees every sealed size; an operator who
joins (share links grant Join) could confirm guesses at an exam's hidden answers, say. It needs
a fresh seal per guess, the signal is noisy within a 16–64 KiB part, and values shorter than the
compressor's shortest match (3–4 bytes, as most numbers stored fixed-width are) barely leak at
all. Rounding sealed sizes up to buckets would not stop it, as the attacker chooses its input's
length; not compressing hosted blocks would.

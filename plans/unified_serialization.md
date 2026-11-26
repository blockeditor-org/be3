# One serialization format, ready to freeze

Depends on: none. Step 6 depends on plans/host_as_platform_adapter.md, step 5 (the block stack
as an instance the root guest spawns), which redraws the hops ops take.

be3 encodes what it saves and sends in several ways, each defined in its own place: WIT
records over postcard (every protocol), `be_block_model` documents as CBOR keyed by name,
registers as serde in CBOR, bincode (be-lib-remap, tabletop), and postcard rows in be-server.
WIT's `with` maps records onto hand-written Rust types whose field order must match by hand,
and a byte field travels as one run only where someone remembered `serde_bytes` (be-block-protocol's
`PushMessage.sealed` did not).

Nothing is frozen and nothing will be when this lands (guides/frozen_formats.md): builds still
refuse each other unless their build ids match. The goal is that freezing later only adds a
marker, with every rule an older build relies on already in place.

## Built: the prototype

`crates/be-wasm-schema` (the format and `#[derive(Schema)]`), `crates/be-wasm-schema-derive` and
`crates/be-wasm-schema-lock` (the lockfile), in the be-wasm family so be-wasm-ops can use them.
Nothing uses them yet.

- A tagged, canonical encoding made to decode fast: numbers fixed-width little-endian (a `u64`
  is 8 bytes), lengths a fixed 4 bytes so nothing written moves, and a list of numbers its
  memory, so it encodes and decodes as one copy on little-endian machines. Fields by id, each
  skippable; a missing field decodes as its type's default where it has one;
  `#[schema(unknown)]` keeps unknown fields or variants and writes them back; `&str` and `&[u8]`
  decode borrowed. Values nest at most `MAX_DEPTH` deep, so untrusted input cannot overflow the
  stack; `usize` and `isize` are refused, as their width differs between wasm32 and 64-bit.
  A fuzz target (`:fuzz-decode`) and a test drive both decoding arbitrary bytes and reading back
  values built from them.
- A must-understand bit in every key (`id << 4 | critical << 3 | wire`), set by
  `#[schema(critical)]` on a field or variant: a reader that does not know such a member refuses
  the value (`Error::NotUnderstood`) rather than skipping or keeping it, so a breaking change is
  never read by an older build as a default, nor written back by one beside its own edit. It is a
  rule every reader follows, so it is built before anything freezes; the first frozen build is
  the oldest reader the next breaking change meets.
- The source is the truth for structure and freezing (`be_wasm_schema::frozen!()` freezes a
  file); `schema.lock` holds the ids, so members reorder freely. Every entry updates
  incrementally; an unfrozen one takes any change. A frozen one refuses a retype, a closed
  enum's new variant, unfreezing, a frozen type that holds an unfrozen one, and a retired
  name given again; removing or renaming a frozen member needs a hand edit of the lockfile
  (marking it `retired`, or renaming it), so no field that meant something goes silently.
  A field added after freezing must decode when missing, which the derive checks at compile
  time.
- buck passes every crate's `schema.lock` to its compiles (buck/cargo/defs.bzl), the derive
  numbers by it as verify will, and be-tool-fix-rust-source rewrites it as one of verify's
  autofixes (`schema_locks.rs`), failing on what a frozen type refuses.

## Steps

1. **Decide the one definition language.** Rust (derives, and an attribute macro over traits
   for interfaces, with a WIT or other description generated for readers) or WIT for
   everything (which needs a dialect for document semantics and field annotations). Record it
   in decisions/. Either way the bindgen or macro reads other crates' types from their
   `schema.lock`, not their source.
2. **Saved data.** Move registers, commits, manifests, metadata and be-server's rows onto
   `Schema`, and `be_block_model`'s stored form onto its encoding, keeping the model's own
   rules (list ids, `Text`, `#[critical]`, kept data). Retire `Kind::registers_accept_missing_fields`
   and `schema::freeze`/`check_frozen` for the lockfile. Signed bytes keep their own encodings
   (decisions/signed_bytes_keep_their_own_encodings.md).
3. **Migrations.** A breaking change to a frozen type, made with code: on the member,
   `#[schema(migrate(from = PersonV1, with = person_from_v1))]`. The lockfile retires the old id,
   keeping its type, and gives the new type a new id, which is must-understand; a frozen retype is
   refused unless it migrates. Decoding takes the new id, else decodes the old as `PersonV1` and
   runs the function; encoding writes only the new. `PersonV1` stays, frozen in a file of its
   own, which the lockfile already guarantees (a frozen type is never removed) and the attribute
   names. Ids are the versions, so no version number travels. `be_block_model`'s
   `#[model(format = N, migrate = path)]` becomes this, migrating the document's root.
4. **Protocols.** The bindgen writes the format in place of postcard; method ids come from the
   lockfile, not hashes of names; `with` becomes a check against the mapped type's lockfile
   entry, and `encodings`, `lenient`, `inline` and `be_wasm_ops::wire`'s byte encodings go.
   Decide whether every protocol variant is open. Reading in place (aligned views, as rkyv or
   Cap'n Proto do) stays possible later, since the format is ours; compression wins size back
   (plans/deferred/compression.md). Local interfaces (the GPU) stay flattened
   wasm calls; the lockfile checks that a frozen one only gains functions.
5. **The rest.** bincode (be-lib-remap, tabletop) and remaining postcard. JSON for files people
   edit and tools' output stays.
6. **Transport.** Encode straight into a guest's memory, decode borrowed in the guest, and
   forward payloads unopened at hops that only relay; measure first which of these matter.
   The host never decodes in place from memory a guest can write while it reads.
7. **Freezing, when the user asks.** First decide the outer message every top-level value
   (a stored document's root, a protocol message) is a field of, so a root can migrate as a
   field does: a value with no parent field has no id to change. Then a handshake that
   negotiates versions in place of the build-id check, and guides/frozen_formats.md and AGENTS.md's principle updated.

## Open questions

- Unknown fields are skipped or kept; a decode that refuses non-canonical input (fields out of
  order) is not built, and hashing only needs encoding to be canonical.
- Retyping a frozen field is refused outright, though some retypes are compatible (`u32` to
  `u64`); allowing those needs a table of them.
- A frozen member's removal is allowed once retired, but nothing checks that newer data
  without it still decodes in an older build (its type must have a default there).
- Types are keyed by name within a crate; two `Schema` types of one name in different modules
  are refused.

# One serialization format, ready to freeze

Depends on: none. Step 7 depends on plans/host_as_platform_adapter.md, step 5 (the block stack
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
Nothing uses them yet. A review found what had to change before anything depends on it; what
became permanent at the first freeze is done, the rest is below.

- **The wire.** A key is a varint of `id << 6 | no_copy << 5 | critical << 4 | wire`, with
  sixteen wire types, six spare: nothing (a unit variant, or a message whose fields are all left
  out), 1, 2, 4, 8 and 16 bytes, a block reference (16 bytes), and, after a 4-byte length,
  bytes, a message, a list (its items' wire type and count first) or packed numbers (their type
  first). So a value can be walked without its schema (`references`, `dump`), and lists of
  numbers say what they hold.
- **Fast one copy.** Numbers fixed-width little-endian, decoding from any narrower width of
  their signedness, so widening a field reads old data and an older build refuses wider data
  rather than misreading it. Packed numbers encode and decode as one copy of their memory;
  `Vec<u8>` and the like are bytes. Lengths a fixed 4 bytes, set once the contents are written,
  so `encoded_len` is known before writing.
- **Evolving.** Fields by id, each skippable; a missing field decodes as its type's default or
  its own `#[schema(default = ...)]`; `#[schema(unknown)]` keeps unknown fields (one pointer
  when there are none) or variants and writes them back. `#[schema(critical)]`: a reader that
  does not know the member refuses the value; `#[schema(no_copy)]`: it may read past it but never
  writes it back. `Kept<T>` holds a value's bytes and decodes them on demand, so a value an older
  build cannot read fails only whoever asks for it.
- **Canonical.** Fields by id, defaults left out, map entries and set elements sorted by their
  encoded bytes whatever holds them, NaN one bit pattern; `decode_canonical` refuses any other
  bytes.
- **Safe on untrusted input.** `MAX_DEPTH` in encoding as in decoding, an error rather than a
  panic past 4 GiB, a list's count checked against its bytes, an allocation budget per decode,
  and errors that carry the path of fields they were found under. A fuzz target
  (`:fuzz-decode`) and a deterministic test drive decoding arbitrary bytes and reading back
  values built from them.
- **Types.** Generics, one lifetime, `Uuid`, `u128`, sets, tuples, `Arc` and `Rc`, `Duration`,
  `NonZero*`, `Cow`; `usize` and `isize` refused.
- **Freezing.** Every type is `#[schema(frozen)]` or `#[schema(unfrozen)]`, and a frozen type
  holds only frozen types, which the compiler checks (`Encode::FROZEN`) across crates.
  `schema.lock` holds the ids of every type, frozen or not, so members reorder freely, updated
  incrementally; an unfrozen entry takes any change. A frozen one refuses a retype, a change of
  `critical`, `no_copy`, a default or the default variant, unfreezing, an enum that does not say
  whether it may grow (a `#[schema(unknown)]` case, or `#[schema(closed)]`), a variant added to
  a closed one, an open one closed, a struct no longer keeping unknown fields, and a retired name
  given again; removing or renaming a frozen member needs a hand edit of the lockfile (writing
  `retired`, which keeps the member's type, or renaming it). A field added after freezing must
  decode when missing, which the derive checks at compile time.
- buck passes every crate's `schema.lock` to its compiles (buck/cargo/defs.bzl), the derive
  numbers by it as verify will, and be-tool-fix-rust-source rewrites it as one of verify's
  autofixes (`schema_locks.rs`), failing on what a frozen type refuses.

## Steps

1. **Freezing checked against compiled descriptors.** The lockfile records each member's type
   as the source spells it, so a type alias, a change in another crate's type, a type a macro
   makes and a hand-written impl all escape it. The derive (and each hand-written impl) gives a
   structural descriptor of the type, built from its members' types' descriptors; a test the
   derive adds per crate compares those with the lockfile, so a frozen type's bytes are checked,
   not its spelling. The same descriptors key types by module path (two types of one name in a
   crate are refused today), let safe retypes be allowed by rule (`u32` to `u64`, a unit
   variant to a struct one), keep the type a retired member had, give the description of the
   protocols for other languages (step 4), and make `dump` schema-aware. Freezing a type then
   also needs golden bytes: a fixture of what it encodes to, which every later build must still
   decode, the only check of a hand-written impl. Also: an adapter for foreign types
   (`#[schema(with = ...)]`), and `Box<str>` and `Box<[T]>`, which decode only once the
   descriptors replace the overlap with `Box<T>`.
2. **Saved data.** Move registers, commits, manifests, metadata and be-server's rows onto
   `Schema`, and `be_block_model`'s stored form onto its encoding. Signed bytes keep their own
   encodings (decisions/signed_bytes_keep_their_own_encodings.md). What the model needs, which
   is more than a move:
   - **Block references** as `Reference`, so `references` finds them in data a newer build
     wrote, for the graph and reclamation.
   - **Parts**: today's CBOR tags (`PART(hash)`, `SPLICE([...])`, UUID tag 37) become a design
     on the wire types, a spare one if a part reference needs its own, and splitting a list
     across parts reckons with fixed 4-byte lengths. The 16–64 KiB thresholds are measured in
     CBOR bytes today; measure them again.
   - **What an unknown critical field locks**: in the model it locks one object, which can
     still be moved or removed; here `NotUnderstood` fails the decode. Objects and registers are
     held as `Kept<T>`, which also gives "an undecodable register keeps its bytes".
   - **Collaboration**: an older peer receiving an edit it does not know keeps it in the
     journal and relays it without applying it, and locks the document; relays forwarding
     payloads unopened (step 7) treat the key's flags as opaque.
   - `marks.rs` holds a `serde_json::Value`, which the format has no equivalent for: design
     the marks' value, or add a dynamic value.
   - About 170 `usize` fields in serialized types, each chosen as `u32` or `u64`, and 18
     hand-written serde impls (`Hash`, `BlockRef`, `Marks`, `Sequence`, `BlockMetadata`, ...),
     each with a descriptor (step 1).
   - Retire `Kind::registers_accept_missing_fields` and `schema::freeze`/`check_frozen` for
     the lockfile, and `read_stored` and other CBOR dumps for `dump`.
   - JSON stays for files people edit, so a type both saved and edited as JSON would carry two
     derives: give `Schema` a JSON text encoding of the same descriptor, so one derive covers
     both, or keep such types apart.
3. **Migrations.** A breaking change to a frozen type, made with code: on the member,
   `#[schema(migrate(from = PersonV1, with = person_from_v1))]`. The lockfile retires the old id,
   keeping its type, and gives the new type a new id, which is must-understand; a frozen retype is
   refused unless it migrates. Decoding takes the new id, else decodes the old as `PersonV1` and
   runs the function; encoding writes only the new. `PersonV1` stays, frozen, which the lockfile
   already guarantees (a frozen type is never removed) and the attribute names. Ids are the
   versions, so no version number travels. `be_block_model`'s
   `#[model(format = N, migrate = path)]` becomes this, migrating the document's root.
4. **Protocols are defined in Rust** (decisions/wit_describes_the_wasm_boundary_and_rust_the_frames.md):
   an `#[interface]` attribute macro over traits takes over from be-wasm-ops-bindgen for our own
   protocols, generating what it generates now (the guest's `Pending` and `Stream` calls, the
   `Guest` traits for exports, the host's `embed::Call`), with each method's id and its
   arguments' and results' ids in the lockfile, so an argument list grows as a record does. The
   descriptors (step 1) give the description of the protocols for other languages, and the
   types the macro makes are numbered through them, not through source verify parses. WIT stays
   only for the wasm boundary (plans/a_component_model_boundary.md).

   What it keeps from WIT, so the protocols stay describable in any language and close to the
   component model:
   - **async funcs, `Stream<T>` and `Future<T>`** as WASI 0.3 has them, as arguments as well
     as results (a stream the caller feeds), back-pressured as the kernel's are;
   - **resources** with constructors, methods and a destructor, owned (`Document`) or borrowed
     for the call (`&Document`), dropping an owned one ending it as canceling an op does;
   - **a closed set of value types**: records, variants, enums, `Option`, `Result<T, E>`, lists,
     maps, sets and `flags` (named bits that can grow), adding `Result` and a `flags` derive to
     be-wasm-schema;
   - **worlds**: what each kind of guest imports and exports, named;
   - **`@since` and `@unstable`** as `#[since(..)]` and `#[unstable(..)]`, recorded in the
     lockfile, for the version negotiation step 8 builds and for parts freezing leaves out;
   - **names independent of Rust**, derived from identifiers by fixed rules, for the generated
     description;
   - **shared ids in one place**, as `be3:shared` is.

   And what the bindgen does today: latest-value `*-watch`es, a dropped `Reply` answering
   `Unanswered`, one `Complete` per op, children canceled with their parent, and `secret`
   arguments zeroed once decoded, including what `Unknown` and `Kept` copy of them, and the
   input a borrowed decode read them from.
5. **Protocols moved.** Every interface in `be-sdk-api`, `be-sdk-type-api`, `be-wasm-root-api` and
   the server protocol rewritten from WIT to Rust (the shapes match one to one, so a one-time
   converter can do most of it), carried in the format in place of postcard. Method ids come from
   the lockfile, not hashes of names; the stack's types are used directly, so `with`,
   `encodings`, `lenient`, `inline` and `be_wasm_ops::wire`'s byte encodings go.
   Rewrite the decisions that describe today's protocols when this lands:
   decisions/guest_calls_are_async_ops_except_local_ones.md (marked superseded),
   decisions/server_protocol_is_unversioned.md (postcard over ops frames) and
   decisions/app_libraries_are_coarse.md (postcard and serde in the base library, where
   be-wasm-schema takes their place).
   Decide whether every protocol variant is open. Reading in place (aligned views, as rkyv or
   Cap'n Proto do) stays possible later, since the format is ours; compression wins size back
   (plans/deferred/compression.md). The GPU is not in this plan: it is wasi:webgpu in the
   canonical ABI (plans/be3_gpu_is_wasi_webgpu.md, plans/a_component_model_boundary.md).
6. **The rest.** bincode (be-lib-remap, tabletop) and remaining postcard. JSON for files people
   edit and tools' output stays.
7. **Transport.** Encode straight into a guest's memory (through `cabi_realloc`, sized by
   `encoded_len`), decode borrowed in the guest, and forward payloads unopened at hops that only
   relay; measure first which of these matter. The host never decodes in place from memory a
   guest can write while it reads.
8. **Freezing, when the user asks.** Before the first freeze, since each is permanent after it:
   - the outer message every top-level value (a stored document's root, a protocol message) is
     a field of, so a root can migrate as a field does, and which changes every top-level
     encoding;
   - fixed-width numbers against varints, measured on real documents and edit streams, with
     compression (plans/deferred/compression.md);
   - `MAX_DEPTH`, measured against how deep a wasm guest's stack can go (a list, a map entry
     and an `Option` each cost a level);
   - which frozen structs must keep unknown fields: required for stored and collaborative
     types, so an older build writing one back drops nothing.

   Then a handshake that negotiates versions in place of the build-id check, and
   guides/frozen_formats.md and AGENTS.md's principle updated.

## Open questions

- Ids of a frozen type can collide across branches: two that each add a field take the same
  id. Harmless while only builds of main write data that outlives them; a preview deployment or
  dev server that keeps data written by another branch would read it wrongly.
- A frozen member's removal is allowed once retired, but nothing checks that newer data
  without it still decodes in an older build (its type must have a default there); the
  descriptors (step 1) can.
- Small ids cost a byte of key each past id 1 (ids 2 to 127 take two); unfrozen types keep
  retired ids, so their ids only grow. Compression recovers most of it.

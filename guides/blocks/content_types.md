# Content types and the model

Part of the block stack (guides/the_new_block_stack.md). To add a type, start
with guides/adding_a_block.md; this is what is underneath it. Types run in type
modules (guides/type_modules.md).

## The content traits

A block's content opts into traits in `be-block`: `BlockContent` (durable
bytes, the references they declare, a name), `LiveEdit` (what a session
carries: `apply`, `rebase`, `absorb_operation`, `child_operations`), `Merge`
(three-way merge, required for offline editing), `Undo` (client-local steps)
and `Streamed` (`[u32 header length][header][payload]`, so a reader takes the
header and then a byte range).

Undo emits inverse operations rather than rewinding state: `Undo::step` is
taken from the state before an operation, and `revert`/`reapply` produce
operations against the state as it is *now*, so an undo leaves alone what
someone else changed since.

`references_in(organization)` is what the peer records for a commit. Text uses
it: a block URL names its organization, and a URL pasted from another
organization is a link, not a reference. The URL format (`be_block::block_url`,
`<base>#block/<organization>/<block>`) is stored in text and so is read
forever: a reference is found by its fragment alone, whatever host precedes it.

### Parts

`BlockContent::parts` returns the stored form as a tree of `Part`s
(`be_block_store::manifest`): new bytes, a `Reused` part of the head the
content was loaded from, or a `Node` of child parts. The leaves in order are
`encode()` exactly, so everything that reads bytes (`decode`, merge, type
modules, export) is unaffected; parts only decide what a save seals and
uploads. Each `Bytes` leaf is sealed on its own (split by the chunker only
past its minimum, 256 KiB by default) and each `Node` becomes a node of the
manifest tree (guides/blocks/server.md), so an edit costs the changed leaf and
the nodes above it. `from_stored(&Stored)` decodes and keeps the `PartRef` of
each `LoadedPart` it may hand back as `Reused`; the tree it gets has the shape
the content wrote, a leaf the vault split over chunks still being one part.
Keep a node's children to dozens, choose parts from the content alone (every
replica must choose the same ones), and leave reuse to parts whose bytes did
not change. The default is one leaf of `encode()`. Peer-level `save` and
`open` use parts. A type module's replica sends its parts' shape with the
bytes (`written.parts`, `be_sdk_type_api::written_shape` and
`written_parts`); it holds no `PartRef`s, so nothing is `Reused`, but sealing
is deterministic, so unchanged parts seal to the chunks the head already has
and only the changed ones upload.

A `Node` also lists the blocks its own bytes reference (`Part::Node {
references }`, read back as `LoadedPart::references`). A commit references
`references_in` and then every node's references in tree order, so content
that keeps references per part reports only the rest from `references_in`,
and a part it reuses keeps its references without being read or encoded.

## Documents: `be_block_model`

Almost every type is a `Document<R>`: structs deriving `be_block_model::Model`
with a `Root` naming the content type. The derive supplies merge, rebase and
undo; `Document<R>` implements the traits in `be-block` (`model.rs`), so it is
registered with `undo` in `be_block_types!`. Content that does not fit can
implement the traits by hand (blobs, below).

A field is a **register** (any serde value, set whole; setting it on both
sides offline conflicts), `Count`, `Grid<T>` (dense cells for pixels; bounds
are part of the value), `List<T>` of `Model` objects, `Map<K, V>` (each key a
register), `Latest<T>`/`LatestMap<K, V>` (last-write-wins by `Stamp`: never
conflict, never undo; `next(time, origin)` stamps past what it replaces, so a
slow clock still overwrites what it read; view state uses them), `Text`,
`RichText` (text plus marks, `be_lib_sequence::marks`) or `Series<T>` (an
append-friendly list of registers stored like a `Vec<T>`, so a register can
become a series without losing data).

How the model works:

- **A table of objects with ids.** Every object in a `List` has an `ObjectId`,
  and edits name the object, not a position: an edit to an object that moved
  still lands on it. `move_into` carries an object and its subtree to another
  list of the same kind; into another kind it does nothing, since saving would
  retype it.
- **Edits.** The derive gives each field a constant (`CalendarEvent::TITLE`)
  that builds `Change`s: `set`, `add`, `put`, `insert`, `move_into`,
  `Change::remove`. Types wrap these in helpers that write only fields that
  changed.
- **Nothing to rebase.** Edits address objects by id and anchor inserts to a
  sibling. A list is a `Sequence` of ids (`Items`) remembering each object's
  slot; an insert takes the next offset of the client that made it
  (`local_client`), so every replica gives it the same slot. A removed object
  leaves a tombstone and reuses its slot if it comes back.
- **Session state.** Tombstones and text positions are never saved. The owner
  hands them over in `Snapshot` (as of its last seal) and `Sealed`
  (`LiveEdit::session_state`), and they go when the session ends.
- **Derived ids.** Anything two peers may create at once for the same purpose
  (a database row past the end, a canvas component for a schema) takes an id
  derived from what it is for, so the second insert is refused and its edits
  land on the first.
- **Offline merge** (`Document::merge`) matches objects by id across the three
  versions and merges each field on its own; list order merges like
  `merge_slices` once moved items are taken out and put back after their
  sibling. You delete what you saw: deleting what the other side edited deletes
  it and counts a conflict; what the other side moved or inserted into a
  removed container survives (back where it came from) and counts a conflict.
- **Undo** (`Document::step`) records an undo and redo change per change, taken
  before the edit. Register undo is conditional (`Change::SetIf`,
  `Change::PutIf`), which is how undo leaves other people's edits alone; a
  move's undo is unconditional; `Change::RemoveIf` removes only what still
  holds what the remover expected. A grid paint is conditional per cell.

Examples worth reading for identity: the browser tab (current page is an
`Option<ObjectId>`, not an index), video (attached clips are objects in the
clip's list, so cycles are refused by the model; `VideoProject::edit_for`
turns commands into edits), the logic grid (wires stored as unit segments so a
cut, extension and branch made at once all survive) and the canvas
(guides/canvas.md). The checklist (`crates/be-type-checklist`) is the type to
copy.

## Sequences

`Text`, `RichText`, `Series` and `List` sit on `be_lib_sequence::Sequence`
(design choices: `decisions/sequence_*`, `decisions/text_*`).

- A position (`Pos`) is `(client, offset)`: each client appends everything it
  inserts to one buffer of its own, so offsets are unique without
  coordination. `ClientId` is per connection, so a reconnect starts a new
  buffer.
- The order is a list of fragments (slices of one client's buffer, visible or
  tombstone) in chunks with visible counts, plus an index from each fragment's
  first position to its chunk. Typing extends one fragment; fragments split by
  deletes are not rejoined until the session ends.
- `SeqOp` applies and reports the visible splices it made, so an editor gets
  exact ranges. `Delete` hides exactly the positions the deleter saw, so text
  inserted inside them concurrently survives; `Undelete` and `Swap` are the
  undos that restore original ids, so anchors keep working.
- **Text on the wire** carries positions only: the receiver fills visible bytes
  from its own copy, and an undelete carries the bytes it brings back. An
  offline merge of `Text` is a line diff3; for `RichText` a line is its bytes
  and formatting together, so formatting a line on one side and typing in it
  on the other conflicts. Marks have ids (a clock and the client); the later
  wins where they overlap; undoing a format removes that mark only.
- **Fuzzing.** `be_lib_sequence::fuzz::sequence` (feature `fuzzing`, libFuzzer
  target `crates/be-lib-sequence/fuzz/sequence.rs`) checks three clients
  against a naive model; `be_block_model::fuzz::lists`
  (`//crates/be-block-model:fuzz-lists`) does a board of columns and cards.
  `./scripts/buck run //:fuzz` runs every target until stopped; a crash becomes
  a test in `crates/be-lib-sequence/src/tests`.

## The stored form

A document's stored form is the CBOR `{"format": N, "root": object}`
(`Document::to_stored`) holding nothing a session needs. An object is a map by
field name plus `$kind` (`#[model(kind = "...")]` or the struct's snake-case
name) and, in a `List`, `$id` (tag 37 UUID; a repeated id gets a fresh one on
load). A register is its serde form in CBOR, and that same CBOR is what
registers hold in memory and what `set` sends.

`Document::to_bytes` writes that value split into a tree of `DocumentPart`s,
each about 16–64 KiB of CBOR (`crates/be-block-model/src/parts.rs`): a CBOR
sequence of the root part, then each part it names with its own parts after
it. `from_bytes` hashes each part and joins them back
(`be_block_model::read_stored` does that alone, for tools that show a
document's CBOR). A parent names its parts by the SHA-256 of their bytes:
`PART(hash)` stands for a list object larger than a part, `SPLICE([hash,
...])` for an array or byte string split across parts (or an index level, past
64 parts). Arrays larger than a part split after items whose key hashes below
a threshold (a list object's key is its id, any other item's its encoding), as
a prolly tree does; byte strings (`Text`, `RichText`, a `Grid`'s cells) split
by content-defined chunking. Encoding is deterministic, kept data included
(its maps are written in canonical key order), so editing, inserting or
removing a list object changes one part plus the parts on its path to the
root, moving one changes two, and typing in a large `Text` changes one.

`Document`'s `BlockContent::parts` saves each part with children as a `Node`
of its own bytes and then its children, and one without as `Bytes`.
`from_stored` maps each loaded part's hash to its `PartRef`, so a part whose
hash didn't change goes back as `Reused` with everything under it.

No type is frozen, so a type's format changes however it needs to, with no
renames or migrations for old data. These are the tools a frozen type would use:

- **Adding is free**: a missing property reads as its default. A struct held in
  a register needs `#[serde(default)]` (on each field of an enum's struct
  variant too), or old documents lose the whole register;
  `Kind::registers_accept_missing_fields` lists offenders.
- **Unknown data is kept** on its object (`Object::kept`) and written back, so
  an older build never drops what a newer one wrote; an undecodable register
  reads as default but keeps its bytes until set.
- **Renames**: `#[model(rename)]`, `#[model(alias)]`,
  `#[model(from_old("old", convert))]`. Old names are read, not written.
- **`#[critical]`** fields: an older build that does not know a critical field
  set away from its default locks that object (`Document::is_locked`) but can
  still move or remove it whole.
- **Reshapes** bump `#[model(format = N, migrate = path)]`; a build refuses a
  newer format.
- **Block references** stored as `BlockRef` (or a `Uuid` with
  `#[serde(with = "be_block_model::references::block_ref")]`) are tagged, so
  `Document::block_refs` finds them even inside unknown properties.

A block saved by another schema is offered for reset (guides/blocks/host.md,
"Blocks saved by another build"). Freezing (`Root::FROZEN`, `schema::freeze`,
`schema::check_frozen`) is guides/frozen_formats.md.

## Files: `Blob<K>`

A small header plus a payload that can be megabytes: `Blob<K>` for any
`BlobKind` (`ImageContent`, `AudioContent`, `PdfContent`,
`GameModuleContent`). It saves its header and payload as two parts and keeps
the payload's ref when loaded, so a rename uploads only the header. It is
`Streamed`, its only live edit is
`BlobOp::SetHeader`, and an offline merge takes whichever side changed. The
payload never travels as an operation (session frames are at most 8 MB): it
arrives as a commit, from `write-content` seeding a new block or replacing an
existing one (guides/blocks/host.md). A blob kind bumps `BlobKind::SCHEMA`
when old headers would be read wrongly.

## Testing a type

Helpers in `crates/be-block/src/tests/`, the model in
`crates/be-block-model/src/tests/`. An editor's tests use
`be_sdk_test::BeuiTest`, whose `ContentStore` hosts every block through one
in-process `be_sdk_type_guest::Module` (`hold::<C>` registers a type it does
not know), plays the host's side of `be3:app/blocks` over be-wasm-ops
(`be-sdk-test/src/link.rs`), and uses the test's frame clock. `edit` and
`apply_elsewhere` are another participant's edits, `set_peers` and
`receive_from_peer` its presence and messages, `keep_history::<C>` turns on
undo, and `store()` hands a fixture the same store.

# One way each in the block stack

Depends on: none.

Mechanisms in the block stack that exist twice or more, one of which (export) has already
drifted into a bug.

## Steps

1. **One export of a subtree to a block file.** be-host-stack's `worker.rs` `export` and
   be-tool-cli's `transfer.rs` `Tracker::download` (behind `be … download`) both walk children
   breadth first, build `transfer::Saved`, refuse types whose bytes do not carry all their parts
   (`whole_bytes`) and read content through `read_contents` and `Modules::readable`. The CLI's
   copy writes `log: Vec::new()`, so `be download` drops a chat's signed log, and it skips the
   host's `owner_only` refusal, its `can_view` filter and the widening to a parent that must be
   exported whole (`exported_whole`). One `transfer::collect(peer, top, modules, open)` in
   be-block-type-host, where `open` lets the host hand in the content of replicas it holds open
   and its derived metadata; both callers use it. Import is already shared
   (`Peer::queue_import`). Test: `be download` of a chat keeps its log, and refuses an
   owner-only block.
2. **The type-module WIT uses the stack's types.** `be-sdk-type-api/wit/types.wit` redeclares
   `journal-entry`, `journal-base`, `journal`, `log-item`, `log-import`, `log-page`,
   `log-search`, `along`, `announcement`, `child-change`, `search-document`, `part-ref`,
   `chunk-ref` and `node-ref`, and the signer enum exists three times
   (`be_block_client::Signer`, `be_block_content::LogSigner`, the generated
   `be_sdk_type_api::Signer`). About 27 struct literals in be-block-type-host's `replicas.rs`
   and `modules.rs` and be-sdk-type-guest's `lib.rs`, and two `From<PartRef>` impls, convert
   field by field. Map these WIT types onto the Rust types with bindgen's `with:`, as
   `commit-id`, `participant`, `session-state` and `resume` already are, and keep one
   `Signer`. The log types move down to a crate be-sdk-type-api may depend on (be-block-content
   itself, or the crate below it that holds `PartRef`), whichever keeps the guest's dependency
   tree small.
4. **One way to read a blob by range.** `be_block_content::Streamed` has one implementor,
   `Blob<K>`, and its only callers, `Peer::stream_header` and `Peer::stream_range`, are used by
   one test. Everything else reads `Loads::FirstPart` and then a span. Fold `Streamed` into
   `Blob` (its `encode_streamed`/`decode_streamed` private to blob.rs), delete `stream_*`, and
   port `a_large_image_streams_without_downloading_all_of_it` to `read_span`.
5. **`be_block_type_host::Host` has no-op defaults.** be-tool-cli's `replica.rs` and the tests'
   `Quiet` write about twelve empty methods (`set_marked`, `owned`, `reshaped`, `known_head`,
   `status`, `described`, `conflict`, `counted`, `leaving`, …). The trait gives them empty
   bodies; only the host overrides them.
6. **One `Saved`.** be-block-client has `peer::Saved` (the result of a write) and
   `transfer::Saved` (a block in a file). Rename the second `SavedBlock`.

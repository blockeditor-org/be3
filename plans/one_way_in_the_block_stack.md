# One way each in the block stack

Depends on: none.

Mechanisms in the block stack that exist twice or more, one of which (export) has already
drifted into a bug.

## Steps

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
3. **One read-change-write loop.** Six loops read a head, change the content, write against
   that head and retry when the write is refused, each with its own attempt count and error:
   `peer/prefixes.rs` (8), `peer/account_block.rs` (4, plus an outer retry on `Contended` in
   `contacts.rs`), be-tool-cli's `tracker.rs` `edit` (8), be-block-sync's `pass.rs`
   `list_children`, be-host-stack's `publish_bytes` and be-block-type-host's `reset`.
   One `Peer::update(block, content_type, make)` (`make` gets the head and its bytes and
   answers the new bytes or nothing) with one attempt count and one `Contended` error, and a
   typed `update_content<C: LiveEdit>(block, ops)` over it. The last two loops deliberately
   resend the same bytes against the new head: that becomes `Peer::update`'s explicit `force`
   form, not a seventh loop.
4. **One way to read a blob by range.** `be_block_content::Streamed` has one implementor,
   `Blob<K>`, and its only callers, `Peer::stream_header` and `Peer::stream_range`, are used by
   one test. Everything else reads `Loads::FirstPart` and then a span. Fold `Streamed` into
   `Blob` (its `encode_streamed`/`decode_streamed` private to blob.rs), delete `stream_*`, and
   port `a_large_image_streams_without_downloading_all_of_it` to `read_span`.
5. **`be_block_type_host::Host` has no-op defaults.** be-tool-cli's `replica.rs` and the tests'
   `Quiet` write about twelve empty methods (`set_marked`, `owned`, `reshaped`, `known_head`,
   `status`, `described`, `conflict`, `counted`, `leaving`, …). The trait gives them empty
   bodies; only the host overrides them.

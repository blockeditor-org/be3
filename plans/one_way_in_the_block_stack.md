# One way each in the block stack

Depends on: none.

Mechanisms in the block stack that exist twice or more, one of which (export) has already
drifted into a bug.

## Steps

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

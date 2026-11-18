# Access logs checked once

Depends on: none. (plans/signed_logs.md step 1, which this needed, landed:
its Merkle tree, heads, and range and consistency proofs are
`be_block_keys::merkle`, shared with this plan.)

## The problem

Access logs (a block's, and the organization's: guides/blocks/keys.md,
"Access logs") are linear hash chains, and every check reads one whole:
`Peer::organization_log` fetches the organization's log from its root on each
call and `be_block_keys::check_log` checks every entry's signature and
rules from the root to the head; `checked_access`, `authority_outside`,
rotation and the server's own checks (be-server's access.rs) do the same for
blocks' logs. A proof path is O(n) too: every entry not shown whole is still
a reduced step. So the cost of one access decision grows with the log's whole
history: an organization with ten thousand members and some churn has tens
of thousands of entries, which is megabytes fetched and seconds of Ed25519
checks (more in wasm), on every decision that needs the organization's log.
Today's logs are small, so nothing is slow yet.

Unlike a chat, a sealer cannot skip entries: any entry may be the revocation
of whom it seals to (Who is served what), so an editor must have seen every
entry once. It need not see each again.

## Shape

- **Check once, keep the result.** A device keeps each log's `Checked` state
  (levels, administrators, last lowering, openings) at the head it checked,
  in the refs database beside `KeptLog`, and checks only the entries after it.
  `check_log` becomes `Checked::extend(entries)`; checking from the root is
  `extend` from the empty state.
- **The server proves the extension.** An access log becomes a Merkle tree as
  signed logs are; a read names the head the device holds and gets the entries
  after it with a consistency proof from that head, so a server that rewrote
  or cut the log is caught as it is today, without the device reading the
  prefix again.
- **Proof paths are O(log n).** A reader below Edit is served the entries it
  needs whole (the same set as today) with inclusion proofs against the head,
  and the count of lowerings after each, instead of a reduced step for every
  other entry. The log's length still shows, as it does now, but nothing else
  of the entries left out.
- **The server checks once too.** be-server keeps each log's checked state at
  its head and checks an appended entry against it, instead of the whole log.

## Steps

1. Landed: `Checked::extend` and `Checked::resume`; a whole log's checked
   state is kept in `KeptLog::checked`, and `checked_access` and
   `checked_organization` check on from it (be-block-keys
   `a_kept_log_checks_only_what_was_appended_and_refuses_a_rewritten_prefix`).
   The device still reads the whole log and hashes its prefix: step 2 drops
   that. A proof path is checked whole each time until step 3.
2. **Access logs as Merkle trees, consistency proofs on read.** Done: a client
   test reads the organization's log twice and the second read fetches only
   the new entries and a proof; a server that served a rewritten prefix is
   reported as it is today.
3. **Proof paths with inclusion proofs.** Done: a reader below Edit of a log
   of 100,000 entries is served the entries naming it and O(log n) hashes;
   every test of `holds`, `current_at` and `outdates` on paths passes.
4. **The server keeps checked state.** Done: an append to a log of 100,000
   entries checks one entry on the server.
5. **Guides.** guides/blocks/keys.md "Access logs" and "Proof paths",
   decisions/editors_are_served_whole_access_logs.md updated.

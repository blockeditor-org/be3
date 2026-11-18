# Signed logs: chat messages outside whole states

Depends on: none. Once it lands, it replaces most of plans/authorship.md and
the "Copying parts" and "Notifications that never go" items of plans/chat.md.

## The problem

A block's content is a whole state. Anyone with Edit (the lease holder
sealing, or a device committing directly) publishes a commit that *is* the new
state, and the server, which never reads content, can only check that it
descends from the head. So content can say who wrote a record (a signature)
but never that a record is *missing*: a removal is signed by nobody, and a
commit of an earlier state is a valid descendant.

Chat wants the opposite shape: an append-only log where each record is its
author's, ordered and timed by someone the author cannot steer, and where a
record can be added (an edit, a reaction, a deletion) but not taken out. Every
gap in plans/authorship.md is that mismatch:

- a whole rollback is a legal commit, and ranks only rise inside live edits;
- a reaction taken back must stay as a signed record, or its removal is
  indistinguishable from someone else's;
- a key's replacement can be judged only against an arrival stamp, and only a
  message has one, which lives in content that a direct commit can rewrite;
- a deletion keeps no stamp, so one signed with an old key is silent;
- `Message::arrival` is content, so a member sealing as lease holder writes
  any arrival.

## What chat does to work around it

Measured on `claude/sweet-planck-4h3wjj`. Chat is about 4,400 lines of product
code (be-type-chat 2,600, be-app-chat 2,300) and 3,800 of tests.

| Workaround | Where | About |
|---|---|---|
| Record chains: `EditSlot`, `Head`, `seq`/`previous` keys, ranked writes, `MAX_SEQ`, `next_seq`, `Head::cut` | chat.rs; ranked `LatestMap` in be-block-model | 300 |
| Signed bytes and hashes for messages, edits, deletions, reaction keys; salted commitment; `Sent` kept by a deletion | signed.rs, chat.rs | 330 |
| Arrival kept in content: `stamped`, `LiveEdit::arrival`, `CLAIMED_ARRIVAL`, `arrived_late` | chat.rs, be-block-content, be-sdk-type-guest | 120 |
| Removal detection: `seen`/`own_previous` priors, `gaps`, `cited`, `kept_matches`, `by_author`, Bloom-filter excuses | chat.rs, ui.rs | 350 |
| Verdicts: `Trust` (Altered, Replaced, ...), `proofs`, `forged_deletions`, `missing_messages`, their rows | ui.rs | 450 |
| Tests of the above | both crates | 800 |

About 1,500 lines of product code and 800 of tests exist only to make a whole
state behave like a signed log, and they still leave the five gaps open.

A second family is about size, not authorship, but has the same root (a whole
state that grows forever): segments, `ChatOp::Close`, the segment index and
Bloom filters, closes merged by parts (`MergeParts`, `weave`, `order_segments`),
`WHOLE_BYTES` false and the copies it refuses (duplicate, export, `be`
transfer, version control re-listing, revision restore), and search over
segments with edits in the header. About 900 lines of product code, plus
`Loads::FirstPart` with parts and `merge-parts` in the type-module protocol,
which only chat uses with more than one part.

## Options

### A. The block system has signed logs

A block may have a **log** beside its content: entries the server orders,
stamps and hashes into a Merkle tree, that no commit can change.

- **Server.** A `block_logs` table per organization and block: sequence,
  appender account, arrival, sealed payload (or its hash once redacted),
  signature, and the entry's leaf hash `H(seq, arrival, author, payload hash,
  signature)` in a Merkle tree over the log, as a transparency log keeps one
  (RFC 6962): a log's head is its size and root hash. A linear chain would make
  linking an old entry to a head cost every hash in between (a million-message
  chat's first page would need about 32 MB of hashes); the tree makes both
  checks logarithmic: an inclusion proof shows an entry is in the head a
  device holds (about 20 hashes for a million entries), and a consistency
  proof shows a new head extends the kept one. One op `append(block, entries)` (Edit, or a future Post level;
  idempotent by an entry id the client picks, as journal submissions are),
  one op `redact(block, seq)` (the appender's account, or Manage: drops the
  payload, keeps its hash, so the tree still verifies), one query
  `log(block, from, limit)` and a `log` topic whose push carries the new
  entries. That is a query, two ops and a topic: the five calls stay. Entries
  are charged to the block's payer; limits per entry and per minute. A commit
  can name a log head (`log-through: seq, hash`) in plaintext; the server
  refuses one that names no prefix of the log. The server can check each
  signature against the account's `recovery_keys.signing` to refuse junk, but
  devices verify on their own (decisions/the_server_never_vouches_for_an_identity.md).
- **Client.** `Peer::append`, kept in the outbox while offline and drained
  like a commit; the refs database keeps each block's log pages and its last
  verified head; each page comes with inclusion proofs against the newest head
  and each new head with a consistency proof from the kept one, so reading any
  page costs that page plus O(log n) hashes wherever it is in the log. A head
  that is not consistent with the kept one is reported as the server cutting
  or forking the log (the accepted limit "Forks" in
  guides/blocks/keys.md, now detectable per device). `verify_authored` takes
  the entry's server arrival for every kind of entry.
- **Host and type-module API.** `type-info.log` says a type has one. A replica
  is told new entries (`replica.logged`) and asks for ranges (`read-log`); an
  editor gets them through its content projection, and appends with one
  `FromEditor` message that the host signs (`sign_authored` over the entry,
  its block and the app's domain) before it goes, so an app cannot forget to
  sign or sign as someone else. Entries are bytes to the protocol, with the
  author, seq, arrival and a verdict beside them: no framework types.
  `BlockContent` gains an optional `LogFold` (how a log entry changes what the
  content shows, for search and descriptions).
- **Chat.** The log holds messages, edits, reactions and deletions, each one
  entry signed by its author. Content keeps what is honestly a shared state:
  title, drafts, settings. What a message reads as now is a fold of the
  entries naming it, which only its author's count. Deleting appends a
  deletion and redacts the message's payload: the body is gone everywhere,
  the tree still proves it existed and who wrote it.
- **Fixes.** All five gaps: no commit touches the log, so rollback is only
  the server cutting it, which every device that saw the head detects; a
  reaction taken back is a later entry and can redact the earlier; every
  entry, deletions and edits included, has the server's arrival, so `Replaced`
  works for all of them; nobody but the server writes an arrival. Gaps in
  `seen` are no longer needed: the log is the order.
- **Costs.** One new storage kind in server, client, host and protocol (about
  2,000 lines with tests, comparable to the journal); a second place a type
  keeps data, which must be stated sharply to stay one obvious way: content
  is what anyone with Edit may rewrite, a log is what its authors add and
  only they may take back. The server learns per entry who appended, when and
  how large (it already learns who submits to a session and when).
- **Others.** Comments on any block, the tracker's history, an activity feed,
  the deterministic game's moves (guides/the_new_block_stack.md, "Not built
  yet": "the stored items would have to carry their own signatures") all use
  the same log.

### B. Chat bypasses block content

The same server pieces, but chat's alone: a `send` target `chat-message` per
block, kept in a server table, read by a chat-only host service; the block
holds settings, drafts and membership.

- **Server and client**: as A, named for chat.
- **Host and type modules**: unchanged; the chat app talks to a new
  `EditorHost` call instead of its replica. Search, export, duplicate,
  version control and unread counts each learn about chat's table by name.
- **Fixes**: the same five gaps.
- **Costs**: a little less than A at first (no type-module API), more after:
  every other type that wants signed records builds a second table, and
  every path that copies or indexes a block special-cases chat. That breaks
  "fix it at its source" and "one canonical way".
- **The mailbox** is no fit for messages: it is per account, so a chat of N
  members stores N copies with no shared order, and letters are deleted once
  taken.

### C. Others considered

- **C1. The server refuses commits that drop signed records.** A commit
  would list, in plaintext, the hashes of the signed records it holds, and the
  server refuse one that drops any. Readers must check the content against
  the list, the lease holder still seals everyone's records, offline merges
  must union two lists, and arrival is still content. It is a log bolted onto
  whole states, with both costs. Rejected.
- **C2. Version control's history.** Rollbacks are visible as commits, but
  commits are sealed by whoever holds the lease, not per author, retention
  thins them, and a check would walk history on every open. Rejected.
- **C3. Keep the journal instead of trimming it** for operations a type marks
  as records. The journal already is server-ordered, stamped and sealed, but
  it is tied to a session's build tag, is numbered by the device offline, and
  is folded into the next whole state. A reuses its sequencer and outbox
  logic, not its table.
- **C4. A Post access level** (plans/deferred/chat_moderation.md): may append
  to the log, not change content. Under A it is one rule on `append`; today
  it cannot be built, since content has no append-only part.

## Chat's features under each option

| Feature | A: signed log | B: chat table | C1: record list |
|---|---|---|---|
| Arrival time | simpler: every entry stamped by the server, the `arrival` hook goes | same as A | unchanged, still content |
| Segments | gone: the server pages the log; `Close`, Bloom filters, merge by parts go | gone, as A | unchanged |
| Unread counts | simpler: a place is a seq; a block's summary can carry its log's last seq, so counts outside the editor come free | as A, but chat-only | unchanged |
| Notifications | simpler: `[append, push]` in one write, durable in the outbox; announcements machinery (chat its only user) can go | as A | unchanged |
| Offline sends | kept: appends wait in the outbox, get their seq and arrival on drain; no merge of messages needed | as A | harder: lists union |
| Export, duplicate | export carries entries with signatures and the block they verify in; duplicate needs a choice (below); no more refusals for parts | each special-cased | still refused |
| Search | kept: a document per log page, re-read when a later entry names it | special-cased | unchanged |
| Version control | a snapshot names the log head; preview and restore show that prefix | special-cased | unchanged |
| Sending state | kept: "Sending" until `append` is answered, "Not sent" when refused | as A | unchanged |

## Recommendation

A. The user's concern is that chat fights the block system; B stops the fight
by leaving, which moves the special cases into search, export, copying and
version control, and leaves every later record-like type to repeat it. A
makes the block system say what chat needs (whole states for what anyone may
rewrite, a log for what each author adds), closes all five authorship gaps at
the source instead of detecting them after the fact, and deletes about 2,400
lines of chat (authorship and segments) for about 2,000 of shared stack that
comments and the tracker can reuse. It fits the principles: one obvious place
per kind of data, entries are bytes on the app protocol, and a log is read
and appended only by the block's members, so it reaches no one new.

## Steps

1. **Server log.** `block_logs`, `append`, `redact`, `query.log`,
   `topic.log`, the Merkle tree (heads, inclusion and consistency proofs), arrival, quota and limits, rules in
   `client::rule`, deleted with the block. Done: be-server tests show appends
   numbered and stamped in order and pushed to followers; a publish leaves the
   log as it was; a redaction keeps the tree verifying; a viewer's append
   and another account's redaction are refused; `every_call_states_who_may_make_it` passes.
2. **Commits name a log head.** `log-through` in a commit; the server refuses
   one naming no prefix. Done: a be-server test refuses a commit naming a seq
   past the log or with the wrong hash.
3. **Peer.** `Peer::append` through the outbox, log pages and the last
   verified head in the refs database, a cut or forked log reported,
   `verify_authored` with each entry's arrival. Done: be-block-client tests
   append offline and drain in order; a server answering a log that does not
   extend the kept head is reported; an entry signed with a replaced key
   after its replacement answers `Replaced`; reading the first page of a
   log of a million entries fetches that page and O(log n) proof hashes.
4. **Type modules and apps.** `type-info.log`, `replica.logged`, `read-log`,
   the editor's append message signed by the host, `LogFold` for search and
   descriptions, export carrying entries. Done: a test type in
   be-sdk-type-guest appends from two replicas and both fold one order; an
   app's unsigned or foreign-signed entry never reaches the server; an export
   reimported elsewhere verifies against the original block id.
5. **Chat on the log.** Messages, edits, reactions and deletions as entries;
   content keeps drafts and settings; deletion redacts; unread places are
   seqs; notifications go as `[append, push]`; search per log page. Done:
   chat's editor tests and be-host-stack's `a_chat_message_*` pass rewritten;
   a host test commits an earlier state of the chat and every message still
   shows; a reaction taken back leaves no reaction and no "altered" row; a
   deletion signed with an old key shows "deleted with an old key"; a host
   test sends with the server cut and the message gets the server's arrival
   once it drains.
6. **Remove what nothing uses.** Segments, `ChatOp::Close`, chat's
   `MergeParts`, ranked `LatestMap`, `LiveEdit::arrival` and
   `CLAIMED_ARRIVAL`, the `Trust::Altered` and gap rows, `seen` and
   `own_previous`, and the announcements machinery if chat was its only user.
   Done: `./scripts/verify` passes and a search of the tree finds none of them.
7. **Docs.** A decision ("A block's records that only grow are a signed log,
   not content"), guides/chat.md, guides/blocks/{server,client,content_types}.md,
   guides/type_modules.md and guides/blocks/keys.md updated; plans/authorship.md
   reduced to what is left (first use of a key, forks), plans/chat.md's
   "Copying parts" and "Notifications that never go" removed. Done: no guide
   mentions segments, ranks or `arrival` slots.

Later, not part of this plan: the Post access level on `append`
(plans/deferred/chat_moderation.md), comments on any block, the tracker's
history as a log.

## Decided

The user chose option A, and these answers (2026-11-18):

- **Duplicate** copies the settings and starts an empty chat; export still
  carries the whole history with its signatures.
- **Redaction** is for the entry's author and Manage only; Edit alone no
  longer removes others' messages.
- **A taken-back reaction** redacts the earlier one, so the log keeps only
  that an entry existed and who wrote it.
- **Verification cost**: a device verifies each entry it reads on its own,
  and ties it to the head it holds with an inclusion proof, so a page anywhere
  in the log (a million-message chat's first page, from a search hit) costs
  that page plus about 20 hashes; a new head comes with a consistency proof
  from the kept one. No device ever reads a whole log to trust a part of it.

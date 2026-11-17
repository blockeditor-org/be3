# A simpler server protocol

Depends on: plans/signed_access.md steps 3d, 3e and 4, for step 3 on (steps 1
and 2 here touch none of their calls).

`be3:server` (crates/be-block-protocol/wit) has 76 operations in 10
interfaces, the session resource's constructor not counted. Eight are
connection-wide server-pushed streams (`blocks.changes`, `account.paired`,
`account.storage-changed`, `account.account-notices-changed`,
`contacts.contacts-changed`, `keys.key-work-changed`, `keys.keys-changed`,
`notifications.notify-modes-changed`), four of which carry nothing and only say
"read it again"; a session's `join` stream is a ninth. Beside the websocket
there is HTTP: the sign-in portal, one-shot calls (`begin-browser-login`,
`poll-browser-login`, `read-invitation`), the long-poll fallback and presigned
S3 object URLs.

Much of the server exists for concepts a client can own: the session lease
(generations, expiry, heartbeats, `clean-at`, and a registry that remembers
empty sessions only for a `clean-at` no client reads), per-connection listing
state in `watch.rs` (`Facts`, `Links` and their generation, `REFUSED_LISTINGS`,
`moved-references` and its digest), contact rows the account seals for itself,
account notices, pending key changes, which organization a connection has
open, and a server-wide `revision` beside the per-organization
`change_sequence`.

What the server keeps, because security or storage charging needs it: accounts
(tokens, agents); organizations (membership and roles, from the signed
organization log); the block graph (parent, references for liveness,
`KnowExists` and backrefs, author, payer, and grants from signed block logs);
content (the compare-and-swap on each block's head, and who may read an
object); keys (wrapped key rows served only to viewers); contacts (only that
two accounts are contacts); sessions (ordering submissions, checking who may
submit or relay); push (fan-out to offline viewers, the endpoint allowlist);
limits (quotas, rate limits).

## Where it ends

Five calls and one stream, beside the greeting and the one-shot sign-in:

- `read(org, query, after, follow) -> page`: every read (about 25 calls today).
- `unfollow(org, topic)`: stops following, leaves a session.
- `write(org, ops) -> results`: every change (about 30 calls), applied
  atomically.
- `send(org, target, payload)`: submissions, relays, pairing, pushes.
- `events() -> stream<event>`: everything the server pushes.
- `ping`, and `device-login(begin | poll)` as a one-shot call.

`org` is an `option<organization-id>`: none for what is the account's (its
mailbox, organizations, recovery key, storage). There is no open organization.

**Queries**: `Blocks(ids, with-heads)` (summaries, and contents and journals
with heads), `Children(parent, after)`, `Tree(entry, cursor)` (what was
`changed-since`), `Roots(Shared | Trash)`, `Backrefs(block)`,
`OpenSettings(block)`, `History(block)`, `Log(block | organization)`,
`Access(block)`, `Keys(blocks, after)`, `KeyWork`, `Revocations(account,
contact)` (what was `removal-revocations`), `RecoveryKey`, `Members`,
`Organizations`, `Invitations(org)`, `Account` (storage and quota),
`SigningKeys(accounts)`, `Mailbox`, `Mutes` (decision (a)), `Link(id)` (the
only query a one-shot call may make unauthenticated, as `read-invitation` is
today), `DownloadUrls(hashes)`, `ServerInfo` (push key, whether FCM is
offered, the server's account, its clock), `Session(block, tag)` (joining:
with `follow`, the session's entries arrive as events until `unfollow`).

**Write ops**: `CreateBlock`, `Publish` (an optional commit, metadata and a
reference change, and an optional `journal-base`; covers `set-metadata` and
`rebase-journal`), `Move` (`set-parent`, with its signed parent record),
`AppendLog` (grant, revocation, administrator, member and, once
plans/signed_access.md step 5 lands, owner records: covers `set-access`,
`set-role`, `sign-member` and a person's `remove-member`), `PutKeys` (covers
`put-keys`, `rotate`, `create-escrow`, `reseal-keys`, and the key arguments of
`create-block` and `set-parent`, which a batch carries beside them),
`HoldObjects`, `ReserveUploads` (answers the URLs `upload-urls` does),
`CreateOrganization`, `CreateAgent`, `DeleteAgent` (an agent's removal by its
maker, in any organization), `RotateAgentToken`, `CreateLink`, `AcceptLink`,
`DeleteLink`, `RemoveContact`, `SetRecoveryKey`, `SetPushEndpoint` (none
removes it), `SetMute` (decision (a)), `DeleteMessage`, `RevokeToken`
(`logout`, with the device's push address).

A batch is applied in one transaction, so a rule is checked once over all of
it: a rotation is whole, a move into another scope leaves every moved key
wrapped under the new scope's key, a removal carries every revocation it
needs. That atomicity is the gain, not speed: calls on one connection are
already answered in order and sent together, so independent ones cost one
round trip (guides/blocks/server.md, "No batch request", which this step
rewrites).

**Send targets**: `Session(block, Ordered)` (needs Edit; numbered into the
journal, what `submit` does), `Session(block, Ephemeral { to })` (needs View;
presence and relays), `Devices(to)` (pairing with the account's other
connections), `Mailbox(account)` (a contact or the account itself; kept until
deleted), `Push(block, mentions, keys)` (what `notify` does).

**Events**: `tree-advanced(entry, sequence)`, `session(block, entry)`, and
`topic-changed(topic)` for `keys`, `key-work`, `mailbox`, `storage`, `members`,
`roots` and `mutes`.

## Prerequisites

Besides plans/signed_access.md (below), work in flight on the integration
branch lands first: kept submissions and submission identity (be-block-client's
session and journal code, and the server's `journal.rs`), which step 1
rewrites around; and the finishing of invitation links
(`finish_links`, contacts and links in be-block-client and be-host-stack),
which steps 3 and 4 move into `AcceptLink`, `CreateLink` and the mailbox.

## Signed access first

`AppendLog` is where plans/signed_access.md already points: each of
`set-access`, `set-role`, `sign-member` and `remove-member` carries a signed
record the server appends to a log once it checks there. What that plan has
left:

- **3d (epochs bound to log heads)** changes what `put-keys`, `rotate` and
  `create-escrow` carry. It lands before step 3, so `PutKeys` is defined once
  over epochs that name their heads.
- **3e (proof paths)** changes what `get-access-log` and `get-organization-log`
  serve to readers below Edit. It lands before step 3, so `Log` is defined
  once over the final serving rule.
- **4 (signed parents)** makes `set-parent` and `create-block` carry a parent
  record. It lands before step 3, so `Move` and `CreateBlock` take it from the
  start.
- **5 (owner transfer)** needs no call of its own once step 3 lands: an owner
  record is one more record `AppendLog` takes. This plan subsumes its protocol
  part; the shell's transfer and its test stay with signed access. Before step
  3, it would add a call that step 3 then removes, so it is better after.
- **6 (guides)** is unaffected.

## Order of work

Each step lands on its own, removes the calls it replaces in the same change
(no parallel forms), and ships client and server from one commit
(decisions/server_protocol_is_unversioned.md), so nothing is migrated: old
data is deleted. The riskiest cut is last.

### 1. Leases derived from the session order

The server writes membership into the session order, and every replica, and
the server, derives the lease holder from it with one function. `claim-lease`,
`heartbeat` and `rebase-journal` go; `Publish` takes an optional
`journal-base`.

- **Membership entries.** Joining, leaving, a change of the participant's
  edit access (a grant or a move while joined: today's `Lease::demote`) and a
  claim are membership entries. The server writes the first three; a claim is
  an ordered send by a participant with Edit (the terminal host's
  `Event::Claim`, a hosted block's host). Each is stamped with the order's last
  number when written (it falls between that submission and the next) and is
  kept in memory only, not in the journal, so numbering stays gapless and
  churn never fills `MAX_JOURNAL_BYTES`. Joining answers the membership as of
  the number it joins at, then follows it. After a server restart every
  session is empty and rebuilds from joins, as today.
- **The holder** at any point is the last claimant still present with edit
  access, else the earliest-joined participant with edit access: what
  `Lease::demote` already picks (`editors.first()`). The build the session
  speaks stays its first participant's. One function in be-block-session
  replaces `Lease`, and the server uses the same one for `submit`'s build
  check and for plans/hosted_blocks.md's host relay kind (the server still
  says who hosts: decisions/the_server_never_vouches_for_an_identity.md, "The
  host lease", unchanged).
- **Sealing.** A seal is `Publish { commit, expected, journal-base }`: the
  journal is trimmed to the base only if that publish wins the head's
  compare-and-swap, in the same transaction, and the base names the commit
  published. The server tells participants in the session's events which head
  holds which number, so `SessionMessage::Sealed` and `relay-kind::seal` go
  too. A trim with no new commit (the last one out restarting for free, a new
  build starting the journal over) needs the head to be the base's commit and
  the sender to be the holder at the order's tip, by the server's derivation.

**Why it is safe.**

- *Two sealers.* Content correctness never rested on the lease: every replica
  applies one order, so any replica's seal through number `n` is the same
  content. Two replicas that both think they hold the lease (one has not yet
  seen a claim or a leave) seal against the same head; the compare-and-swap
  takes one, the other is `Rejected` and trims nothing, then sees the new head
  and, no longer the holder, leaves the merge. A trim cannot outrun its commit,
  and the window today's guide describes (the server took the seal but the
  holder stopped before `rebase-journal`) closes. A base that goes back is
  refused as today, but for the existing start-afresh rule. No generation is
  needed for a sealing publish; the holder check covers only trims with no
  commit.
- *A holder that goes offline without a leave.* Only the server writes leaves,
  and it is on every path: it writes one when the socket closes, and when the
  connection goes silent past `IDLE_TIMEOUT` (90 s), as `leave_all` runs on a
  disconnect today. Until then the holder stays the holder. That is no worse
  than now: replicas claim only when the holder is no longer a participant
  (`claim_if_abandoned`), so today's 30 s expiry already moves no lease while
  a silent connection stays a participant; it matters only to an explicit
  claim, which now takes the lease at once.
- *A timeout without `clean-at`.* No client reads `SessionState::clean_at`;
  the server keeps it only to decide whether to remember an empty session
  (`Sessions::settle`), so the registry's empty-session memory goes with it.
  The only timeout left is the connection's.
- *A stuck replica on a live connection* keeps the lease, now and before: a
  holder that stops heartbeating today keeps it too, since nobody claims from
  a participant. Any editor can claim, and a claim is never wrong, only
  repeated work.

The argument holds. Accepted: an explicit claim no longer waits for the old
holder to lapse, so two devices that claim in turn pass the lease back and
forth.

Removed: `claim-lease`, `heartbeat`, `rebase-journal`, `session-state`'s
`generation` and `clean-at`, `relay-kind::seal`, `Lease`'s expiry and claims,
`ServerConfig::lease`, `LEASE_MILLISECONDS`, the registry's empty-session
memory (`EMPTY_SESSION_MEMORY_MILLISECONDS`, `MAX_EMPTY_SESSIONS`), and the
replica's heartbeat (`beat_at`, `Effect::ReportClean`, `HEARTBEAT_INTERVAL`).

Done: be-block-session unit tests where replicas fed the same order with
membership entries interleaved agree on the holder at every number; a
be-block-client test where two devices seal against one head and only the
winner's trim is kept; one where the holder's socket goes silent and the next
editor seals once the server writes its leave; one where a claim moves the
lease and the old holder's next seal is rejected. The host tests that edit
together still pass. Guides: guides/blocks/client.md ("Live sessions", "The
journal"), guides/blocks/server.md ("Live sessions", "The order"),
guides/type_modules.md (the replica's lease and heartbeat).

### 2. Following whole trees, with one organization sequence

A connection follows trees, not listings; the server tells it "tree X
advanced to sequence N" and the client reads `changed-since` from its cursor.
The durable `organizations.change_sequence` replaces the server-wide
`revision`.

- **What a client follows** is an entry point: a top-level block, or a block
  someone can view whose parent they cannot (Shared with me). The hub keeps,
  per connection, only the set of entries it follows (bounded, as listings are
  now), and on each change sends `tree-advanced` to the followers of the
  change's top-level block and of every granted block on its path, latest
  value per entry, so a burst is one event.
- **Departures.** `block_departures` is keyed by the top-level root today, and
  a move within one tree writes none. A reader following a granted subtree
  must hear of a block moved out of it within the same tree, so a departure is
  written for the top-level root and for every block on the old path that
  carries a grant (those are the only entry points below the top). That is
  bounded by the grants on one path, not the depth.
- **Reading.** `changed-since` takes an entry rather than a top-level root,
  and filters by the entry's subtree. Summaries carry the block's `changed`
  sequence instead of `revision`; replies and events are ordered by it, per
  organization. A client still reads only what it shows: a listing
  (children, backrefs, open settings) is a plain read answering the sequence
  it was read at, and the mirror applies a `changed-since` page only to the
  blocks it holds, so the host still never lists the whole organization.
- **One block, its heads.** The `block` listing goes; an open block's tree
  advances with each publish, and a live session's events say which head
  holds which number.
- **Roots.** Shared with me and the trash change when a grant or a move does;
  the account hears `topic-changed(roots)` and reads them again.

Removed: `unlist`, `listing.block`, list-blocks' registering (it stays a plain
paged read until 3a turns it into queries), the `changes` stream's
`block-change`, `block-changed`, `changed-references`, `moved-references` and
its digest, `block-summary.revision`, `listed.revision`,
`revision_floor`, `Facts`, `Links`, `REFUSED_LISTINGS`,
`MAX_LISTINGS_PER_CONNECTION`, and the client's `Held` and `hold_with`. The
`changes` stream carries `tree-advanced` until 3a folds it into `events`.

Done: be-server tests where a follower of a granted subtree hears a block
moved elsewhere in the same tree leave it and a follower of a sibling subtree
hears nothing; where a hundred publishes in a burst reach a follower as at
most a few `tree-advanced` events; where a reply read before a change and an
event after it are put in order by sequence across a server restart. A
be-host-stack test where a folder another device fills is seen through
`changed-since` with only the shown blocks mirrored. Guides:
guides/blocks/server.md ("Announcements and listings", "Changes since"),
guides/blocks/client.md ("Connection"), guides/blocks/host.md ("The graph
mirror").

### 3. The calls merge

In two pieces, each landing whole.

- **3a. read, write, send and events, each naming its organization.** The
  interfaces become the calls above, with every query, op and target stating
  its rule once (`client.rs`'s `rule`, now per variant), and the dispatcher
  applying it before the handler, as today. A batch fails whole and names the
  index of the op that failed and why. Every call names its organization, so
  `open-organization` goes, and what it answered moves: the connection's
  client id to `authenticate`'s answer (the greeting's, after 3b), the role,
  kind and escrow chain to `Organizations`, the server's account and clock to
  `ServerInfo`, the pinned escrow to `RecoveryKey` until step 4. `AppendLog`
  takes every record kind; the unsigned `remove-member` waits on decision
  (b), and notify modes on decision (a). `collect-detached` goes: nothing but
  two be-block-client tests calls it, and
  decisions/garbage_collection_waits_for_trash_and_history.md keeps
  collection off anyway; collection, if wanted, becomes a server policy, and
  those tests go with it. The streams become `events`. Done:
  `every_call_states_who_may_make_it` (a be-server test; there is no decision
  of that name) enumerates queries, ops and targets rather than methods and
  fails until each new one has a rule; a test where a batch whose second op is
  refused writes nothing of its first; one where a rotation missing one
  block's key is refused whole; one where a connection reads two
  organizations without reconnecting; the be-host-tests that sign in, share,
  invite and remove a member pass unchanged.
- **3b. Signing in in the greeting.** The token goes in the greeting, and the
  request's Authorization header stands in when the greeting carries none
  (`be` behind a proxy that injects it, `Credentials::Header`), so
  `authenticate` goes. Header-only authentication, as proposed, cannot work:
  a browser cannot set a websocket's Authorization header, and the host,
  the web build included, signs in with a token in a frame
  (`Credentials::Token`). `logout` is `RevokeToken` from 3a. Done: a test
  where a greeting with a bad token is closed before any call; the web build
  signs in (its host test).

Guides: guides/blocks/server.md ("Protocol", "Organizations and access",
"Accounts", "Notifications"), guides/blocks/client.md ("Connection"),
guides/blocks/keys.md, guides/tracker.md (`be`'s header sign-in).

### 4. The account's own block, and one mailbox

- **An owner-only block** in the personal organization holds what the account
  seals for itself: contact records, its cards and link codes, the escrow keys
  it pinned per organization. `put-contact`, `list-contacts`, `list-links`,
  `pin-escrow` and `own-sealed` go; the server keeps only that two accounts are
  contacts. It is owner-only (decisions/owner_only_settings_are_never_escrowed.md),
  so it is never escrowed, and the personal organization has no
  administrators. A new recovery phrase reseals its key to the new identity
  and starts a new key epoch for it, so the old phrase reads nothing written
  after.
- **The mailbox** replaces pending key changes (`contact-row.changes`,
  `key-change`), accepted links' cards, account notices and the pairing stream:
  `Mailbox(account)` sends to a contact or to the account itself, kept until
  deleted. Notices are the server's, and `DeleteMessage` refuses one in its
  first `NOTICE_HOLD` (a week); a device hides it for itself meanwhile, as
  today. Pairing between live connections stays a send to `Devices`.
- **Bootstrapping.** Working in a company organization needs the escrow key it
  pinned, now in a block of the personal organization: simple only once 3a
  dropped the open organization, which is why this step follows it.

Done: a test where a contact made on one device is a contact on a second
device that reads the account's block, and the server's database holds no
sealed contact record; one where a new phrase leaves the old one unable to
read the block's next commit; one where a notice under a week old refuses
deletion and one older deletes; one where a key change reaches a contact
offline and is read from the mailbox. Guides: guides/blocks/keys.md
("Contacts", "Identity, new phrases and devices"), guides/blocks/server.md
("Accounts"), guides/settings.md (owner only).

### 5. References leave summaries

The server keeps references for liveness, `KnowExists` and backrefs, and
`Publish` still carries the reference change. Summaries stop carrying them:
clients take links and a folder's order from decrypted content and the tree's
shape from parents. About 68 sites read `.references` in be-block-client and
be-host-stack, among them `Graph::holds_in` and the mirror's `References`
query. The risk is a view that shows a folder's children in order without
opening the folder's content, which must now decrypt it, and an offline
device, which has only the content it kept. `LARGEST_SUMMARY_BYTES` loses
`MAX_REFERENCES` ids, and the page budgets sized around it shrink.

Done: a be-host-stack test where a folder's order comes from its content with
the server's summaries carrying no references; a be-server test that a summary
names none and a referenced block still reads as `KnowExists`; the host tests
that list, reorder and link blocks pass. Guides: guides/blocks/server.md
("Answers that fit a frame"), guides/blocks/host.md ("The graph mirror").

## Decisions for the user

- **(a) Notify modes.** Moving them to the device alone means a muted block
  still wakes a phone for every push, which costs battery and shows the push
  service traffic for a block the person muted. Recommended: keep one small
  server-read mute: `SetMute(block, mode)`, the `Mutes` query and its topic,
  replacing `notify-modes`, `set-notify-mode` and `notify-modes-changed`. The
  server's rows stay the one copy; they are not also put in the account's
  block.
- **(b) Removing a member no record names.** A person accepting an invitation
  joins before any administrator's device signs a member record for them
  (plans/signed_access.md 3a-members), so `remove-member` without a record
  stays, and does not fit `AppendLog`. Recommended: sign every member in when
  they accept. The administrator who makes an invitation appends an invitation
  record to the organization's log naming a public key derived from the link's
  code; accepting appends a member record signed with that key and naming the
  accepter's keys. The server never holds the code, so it cannot forge the
  record, and no administrator need be online. Then every removal of a person
  is an `AppendLog`, and an agent's is `DeleteAgent` (the personal
  organization has no log, so an agent there has no record to sign it out).
  Interaction: the members dialog work on `claude/members-sign-in-wip` shows
  administrators the members no record signed in, with a Sign in action. Once
  accepting signs a member in, and with old data deleted, nobody is unsigned
  but a member the server added itself; the list stays as that warning and
  its Sign in action goes. Land that work first, and remove the action in
  step 3b if (b) is taken.

## Trade-offs

- **Re-reading.** Following trees costs one `changed-since` read per tree per
  burst, and each page carries changes in folders the device does not show,
  which it drops. Listings sent only what a connection had listed.
- **Rules per variant.** Who may make each query, op and target is still
  stated once each, and errors are per op, inside one call; the checking code
  stays. Only the dispatch shrinks.
- **What actually shrinks** is not the operation count (76 to about 8, which
  mostly renames) but the server state and code steps 1, 2 and 4 remove: the
  lease and its timers, the hub's per-listing state and reference digests,
  the second revision counter, the open organization, the contact, link and
  notice tables and their four "read again" streams, and `collect-detached`.
- Nothing in decisions/ rules out batching or these merges; "No batch
  request" in guides/blocks/server.md is a guide's note about round trips,
  which step 3 rewrites.

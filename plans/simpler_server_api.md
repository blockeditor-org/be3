# A simpler server protocol

Depends on: none.

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

Much of the server exists for concepts a client can own: contact rows the
account seals for itself, account notices, pending key changes, and which
organization a connection has open.

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
reference change, and an optional `journal-seal`; covers `set-metadata` and
`rebase-journal`), `Move` (`set-parent`, with its signed parent record),
`AppendLog` (grant, revocation, administrator, member and owner records:
covers `set-access`, `set-role`, `sign-member` and a person's `remove-member`), `PutKeys` (covers
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
left, and what it has built that touches these calls:

- **3d (epochs bound to log heads)** is built: a scope or data key's epoch
  names its heads inside what is sealed or wrapped (`CarriedKey::heads`), so
  `put-keys` and `rotate` carry nothing new, and `create-escrow`'s escrow key
  names the organization log's head in the clear, signed and checked by the
  server. `PutKeys` takes them as they are.
- **3e (proof paths)** is built: `get-access-log` and `get-organization-log`
  serve each entry as a `be_block_keys::Step`, whole or reduced, and decide
  which by the caller's access and who sealed it a key, with no new argument,
  so `Log(block | organization)` takes that serving rule as it is.
- **4 (signed parents)** is built: `set-parent` carries the mover's parent
  record (`placement`) and `create-block` the creator's after the root, each
  appended as a grant is, so `Move` and `CreateBlock` take them as they are, and
  `AppendLog` could take a move's record too. `get-access-log` serves a block
  to a caller viewing it or a block below it (a proof path of an ancestor), so
  `Log(block)` takes that rule.
- **5 (owner transfer)** is built with no call of its own: `set-access`
  carries the owner record (`Record::Owner`) as it carries a grant, so
  `AppendLog` takes it as one more record.
- **6 (guides)** is unaffected.

## Order of work

Each step lands on its own, removes the calls it replaces in the same change
(no parallel forms), and ships client and server from one commit
(decisions/server_protocol_is_unversioned.md), so nothing is migrated: old
data is deleted. The riskiest cut is last.

### 1. Leases derived from the session order (done)

The server writes membership entries (join, leave, a change of edit access, a
claim) into each session's order, in memory, stamped `after` the order's last
number and counted by `index`; the server and every replica derive the holder
with `be_block_session::Membership::holder`. A seal is `publish` with a
`journal-seal`, trimmed in the publish's transaction and told as
`session-event.sealed`; a trim with no new commit is a publish of the head on
itself, from the holder. `claim-lease`, `heartbeat`, `rebase-journal`,
`relay-kind::seal`, `SessionMessage::Sealed`, the lease's generation, expiry and
`clean-at`, `ServerConfig::lease` and the registry's empty-session memory are
gone (guides/blocks/server.md, "Membership"). Where it differs from the plan:
a claim is the session call `claim` until 3a makes it an ordered send; a seal
names the build that made it and the run of the order it seals
(`journal-seal`), so one kept in the outbox and drained before its replica
rejoins still trims, and a stale one (an earlier run, another build) publishes
untrimmed; the holder is the last claimant or earliest editor among those that
run the session's build; and `relay-kind` and the replica's `delivery` are gone
with `seal`, since only presence and channel messages are relayed. `journal_for`'s
seal walk (guides/blocks/host.md, "Opening again") stays: step 1 removes the
lost rebase it was written for, and the backlog's "A lost seal's rebase with a
merge above it" says what would show it unreachable.

### 2. Following whole trees, with one organization sequence

Landed: a connection follows entry points (`block-summary.entry`) with
`changed-since`'s `follow`, hears `tree-advanced` and `roots` on `changes`, and
orders summaries by `changed`, the organization's sequence
(guides/blocks/server.md, "Listings and following trees"). The `changes` stream
carries them until 3a folds it into `events`, and `unfollow` becomes 3a's
`unfollow(org, topic)`.

Gap kept: a block a reader knows only exists (through a reference) names no
entry, so it is read again only on `roots` or a reconnect. (One deleted for
good while known that way no longer happens: 3a removed `collect-detached`.)

### 3. The calls merge

In two pieces, each landing whole.

- **3a. read, write, send and events, each naming its organization (done).**
  `be3:server` is one interface, `server` (crates/be-block-protocol/wit/
  server.wit): `ping`, `device-login(begin | poll)`, `authenticate`,
  `read(organization, query, follow) -> page`, `unfollow(organization,
  topic)`, `write(organization, ops) -> list<done>`, `send(organization,
  target, payload)` and `events() -> stream<event>`; the other files keep only
  records. Every query, op, target and topic states its rule once
  (`query_rule`, `op_rule`, `target_rule`, `topic_rule` in client.rs), which
  the dispatcher applies against the organization named; there is no open
  organization, and a connection works in an organization from its first call
  naming it (`WatchHub::join_organization`). A write of more than one op runs
  in one SQLite transaction (`ServerStore::batch`, savepoints for the
  transactions nested in it), each op's rule applied as it comes, and a
  refusal names the op's index (`write-error`). What `open-organization`
  answered moved: the client id to `authenticate`, the role, kind and escrow
  chain to `organizations`, the server's account, clock, push key and FCM to
  `server-info`, and the pinned escrow is the recovery key's `own` (kind
  `escrow`). `collect-detached` and the server's whole delete-for-good path
  are gone, with their tests. Streams became `events`: `tree-advanced`,
  `session`, `paired`, and `topic-changed` for `keys`, `key-work`, `roots`,
  `storage`, `notices`, `contacts` and `notify-modes`, each followed by reading
  its query with `follow` (a peer follows them as it attaches,
  `Peer::follow_topics`); a topic tells no value, so key work and storage are
  read again. Where it differs from the list above: listings stay one
  `list(listing, after)` query (`Children`, `Roots`, `Backrefs`,
  `OpenSettings` are its `Listing` cases); `blocks(blocks, contents, held)`
  answers summaries or contents; a signed-in `link` and the anonymous
  `invitation` are two queries; `set-metadata` stays an op beside `publish`,
  and `create-block` and `move` still carry their keys; `append-log` takes a
  grant or an owner record (`grant`, as `set-access` carried both), a role or
  a member record; `put-keys` is `rows`, `rotate`, `escrow` or `reseal`, each with its own
  rule; notices, contacts, links, `put-contact` and `pin-escrow` wait for step
  4; `remove-member` and the notify modes (`notify-modes`, `set-notify-mode`,
  `topic.notify-modes`) are as they were, decisions (a) and (b) unanswered;
  no `members` topic, since nothing pushed one before. Tests:
  `every_call_states_who_may_make_it` (by case of query, op and target),
  `a_write_whose_second_op_is_refused_keeps_nothing_of_its_first`,
  `a_rotation_missing_one_blocks_key_is_refused_whole`,
  `one_connection_reads_two_organizations_without_reconnecting`.
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

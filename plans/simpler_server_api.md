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

Besides the signed access logs (below, built), work in flight on the integration
branch lands first: kept submissions and submission identity (be-block-client's
session and journal code, and the server's `journal.rs`), which step 1
rewrites around; and the finishing of invitation links
(`finish_links`, contacts and links in be-block-client and be-host-stack),
which steps 3 and 4 move into `AcceptLink`, `CreateLink` and the mailbox.

## Signed access logs

`AppendLog` is where the signed access logs (guides/blocks/keys.md, "Access
logs") already point: each of
`set-access`, `set-role`, `sign-member` and `remove-member` carries a signed
record the server appends to a log once it checks there. What is built that
touches these calls:

- **Epochs bound to log heads**: a scope or data key's epoch
  names its heads inside what is sealed or wrapped (`CarriedKey::heads`), so
  `put-keys` and `rotate` carry nothing new, and `create-escrow`'s escrow key
  names the organization log's head in the clear, signed and checked by the
  server. `PutKeys` takes them as they are.
- **Proof paths**: `get-access-log` and `get-organization-log`
  serve each entry as a `be_block_keys::Step`, whole or reduced, and decide
  which by the caller's access and who sealed it a key, with no new argument,
  so `Log(block | organization)` takes that serving rule as it is.
- **Signed parents**: `set-parent` carries the mover's parent
  record (`placement`) and `create-block` the creator's after the root, each
  appended as a grant is, so `Move` and `CreateBlock` take them as they are, and
  `AppendLog` could take a move's record too. `get-access-log` serves a block
  to a caller viewing it or a block below it (a proof path of an ancestor), so
  `Log(block)` takes that rule.
- **Owner transfer** has no call of its own: `set-access`
  carries the owner record (`Record::Owner`) as it carries a grant, so
  `AppendLog` takes it as one more record.

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

In three pieces, each landing whole.

- **3a. read, write, send and events, each naming its organization (done).**
  `be3:server` is one interface, `server` (crates/be-block-protocol/wit/
  server.wit): `ping`, `device-login(begin | poll)`, `authenticate` (until 3b),
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
  `invitation` are two queries; `set-metadata` stayed an op beside `publish`,
  and `create-block` and `move` carried their keys, until 3c; `append-log` takes a
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
- **3b. Signing in in the greeting (done).** A client's greeting carries its
  token after its build (`be_block_protocol::Greeting`, past a zero byte, so
  another build still reads only the build), and the request's Authorization
  header stands in when it carries none (`be` behind a proxy that injects it,
  `Credentials::Header`); `authenticate` is gone. The server's greeting answers
  who signed in or why not (`Welcome`, `Client::greeted`), and a connection it
  does not sign in is closed before any call. `Connection::connect` takes the
  `Credentials` and answers the connection with who signed in; be-block-client's
  `Peer`, the host (`link.rs`, native and web), `be` and the tests all sign in
  that way. Header-only authentication, as proposed, cannot work: a browser
  cannot set a websocket's Authorization header, so the host, the web build
  included, signs in with a token in the greeting (`Credentials::Token`).
  `logout` is `RevokeToken` from 3a. Where it differs: an HTTP long-poll
  session exists only once the post that greets signs in, so the HTTP
  sessions not signed in (`MAX_UNSIGNED_HTTP_SESSIONS`, their eviction) and
  `ServerConfig::sign_in_timeout` are gone; a websocket counts as not signed in
  only until its greeting (`HANDSHAKE_TIMEOUT`). Tests:
  `a_greeting_that_signs_nothing_in_is_closed_before_any_call`,
  `proxy_account_authentication_never_discloses_the_token`,
  be-tool-cli's `be_signs_in_with_only_an_injected_authorization_header`, and
  the web build's host test.
- **3c. One form for keys and metadata (done).** `create-block` and `move`
  carry no keys: a write puts `put-keys` beside them (`[create-block,
  put-keys(rows)]`, `[move, put-keys(rows)]`), and what it must keep whole is
  checked once, at its end, over everything it wrote (`keys::Whole`,
  `ServerStore::check_whole`): a block made names a data key the write stored,
  and a move into another scope leaves every key it took there opening from
  the new scope's latest key (`KeyMissing` otherwise). `set-metadata` is a
  `publish` with no commit (`commit: option<new-commit>`), which sets the
  metadata and the reference change alone. be-block-client sends them
  (`Creation::ops` for `Peer::create_block`, the outbox and imports;
  `send_parent`; `Peer::define` and `drain_define`), with
  `be_block_protocol::write_all` for a write answered as its first op. Where
  it differs: a publish with no commit is answered with the block's summary
  (`done.block`, as `set-metadata` was), and a head other than `expected`
  rejects it as any publish is (`published(rejected(head))`, read as
  `be_block_protocol::Described`), so `block-has-content` is gone; it
  carries no journal seal, so a trim stays a publish of the head on itself.
  `create-block` still names its key (`key`), set on the block at once so its
  answer's summary names it, and the end checks its row. A write that makes or
  moves a block is a transaction even of one op. The move check skips what
  takes no key anywhere new (a move within one scope, to the top or the trash,
  of an owner-only block, or into a scope with no key) and what the mover may
  not view, as `Peer::rescope` does, which looks at every block below once the
  scope changes (not only when the moved block's key is not wrapped there
  yet) and wraps a nested scope moved out of an owner-only block too. Only the
  scope's latest key counts, so a block restored from the trash into a scope
  rotated meanwhile is wrapped again. The check makes a few queries per block
  moved inside the write's transaction, as `rescope` lists and wraps each:
  bounded by the subtree, which a large move into another scope already
  costs. The client sends no `[move,
  put-keys(rotate)]`: a move out of a scope still leaves the rotation to key
  work (`lost_below`, `Peer::tend`), which the server allows in one write but
  nothing needs yet. Storing a new block's keys through `put-keys` tells
  `topic.keys` to its viewers, which `create-block` did not. Tests:
  `a_block_made_with_no_key_in_its_write_is_refused`,
  `a_move_missing_one_blocks_wrap_under_its_new_scope_is_refused_whole`,
  `a_publish_with_no_commit_changes_the_metadata_and_leaves_the_head`,
  be-block-client's
  `a_block_moved_back_into_a_scope_takes_what_was_made_below_it_meanwhile` and
  `a_block_restored_into_a_scope_rotated_while_it_was_in_the_trash_is_wrapped_under_its_new_key`, and
  `every_call_states_who_may_make_it`'s `publish (no commit)`.

Guides: guides/blocks/server.md ("Protocol", "Organizations and access",
"Accounts", "Notifications"), guides/blocks/client.md ("Connection"),
guides/blocks/keys.md, guides/tracker.md (`be`'s header sign-in).

### 4. The account's own block, and one mailbox

In pieces, each landing whole: the mailbox first (4a, done), the account's own
block with its contacts' records (4b1, done), then its cards and link codes, the
escrow keys it pins, and bootstrapping (4b2), which the bullets below the first
two describe.

- **4a. The mailbox (done).** `query.mailbox`, `topic.mailbox` and
  `op.delete-message` (crates/be-server/src/store/mailbox.rs): each `message`
  names its sender (none for the server's own) and a `letter`, kept until a
  device of the account deletes it. Account notices are `letter.notice`
  (`query.notices`, `dismiss-notice`, `topic.notices` and the
  `account_notices` table are gone; what stays of `store/account_notices.rs`
  is the history of ways to sign in, now `store/identity_changes.rs`), and
  `delete-message` refuses one in its first `NOTICE_HOLD`, where
  `dismiss-notice` changed nothing. Pending key changes are
  `letter.key-change` (`contact-row.changes` and the `key_changes` table are
  gone): a device follows them from its record and deletes them with the
  record written, which `put-contact` now writes only over the record it
  read (`replaces`, refused `record-changed`), so a device that read fewer
  changes than another never rolls the record back. Accepted links' cards are `letter.accepted`
  (`own-link.card` and `links.accepter_card` are gone): `finish_links` takes
  the card from the mailbox and deletes the link and the message in one
  write. Where it differs: there is no `Mailbox(account)` send target yet.
  Every letter today is left by the server in the transaction of the write it
  tells (`accept-link`, `set-recovery-key`, a change to the ways to sign in),
  so none is lost between a write and a later send, and no client sends one
  alone; a target waits for the first letter a client writes on its own (4b's
  own-block work may want one). Pairing stays `target.devices` and
  `event.paired`, as only live connections pair. A block's agent reads and
  deletes from its mailbox, where its contacts' key changes arrive. Removing a
  contact deletes what each left the other, with the links between them.
  Letters are bounded per kind and sender (`MAX_LETTERS_FROM_ONE`): past it an
  accepted card is refused, but a key change lets the sender's oldest go, so
  a contact that never reads cannot hold up a new phrase; the reader's device
  then finds the chain broken and lists the contact as not confirmed, with
  its verified signing keys superseded, until a new link. The host follows
  its contacts' waiting changes just before it takes a new phrase
  (`accounts::set_recovery_key`), so its own changes seal to their newest
  keys (plans/backlog.md, 18, has the race left). Tests: be-server's
  `a_notice_under_a_week_old_refuses_deletion_and_one_older_deletes`,
  be-block-client's
  `a_key_change_reaches_a_contact_offline_and_is_read_from_its_mailbox` and
  `a_device_that_read_fewer_key_changes_never_rolls_a_contact_back`,
  `a_contact_whose_key_changes_outran_the_mailbox_is_not_confirmed_until_linked_again`,
  be-server's `a_key_change_past_the_bound_lets_the_oldest_go_but_an_accepted_card_is_refused`
  and `a_link_accepted_then_unmade_as_a_contact_leaves_no_link_or_card`, and
  `every_call_states_who_may_make_it`'s `mailbox` and `delete-message`.
- **4b1. The account's block, with contacts (done).** `be_block::account_block`
  (`AccountBlock`), owner-only at the top of the personal organization at an id
  derived from the account, made only there and so by the server
  (`blocks::require_account_block`), holds the account's contact records; a
  device reaches it through `be_block_client::AccountRecords`, a peer of the
  personal organization of its own that holds this block's keys alone and
  keeps nothing on the device. `put-contact`, `query.contacts`,
  `topic.contacts`, `contact-row`, `own-kind.contact` and `record-changed` are
  gone; `contacts` keeps two columns. Where it differs: the server can no
  longer write a record, so a record arrives as a letter the account sealed for
  itself (`letter.contact`, left by `accept-link` for the accepter and by
  `create-agent`, now with the maker's `record` of the agent, for both) and a
  removal as `letter.removed` (left to both by `remove-contact`); a device
  takes them into the block in mailbox order and then deletes them, which
  tells its other devices (`topic.mailbox`) that the block changed, so no
  `Mailbox(account)` target was needed. A record's `through` names the newest
  letter it reflects, so a device that stops between writing the block and
  deleting the letters never follows a change twice nor takes back a removal;
  the block is written only over the head read, which replaces `put-contact`'s
  `replaces`. An agent, in no personal organization, has no block and keeps its
  letters, so after its maker's seventeenth new phrase its chain of key changes
  breaks for good (keys.md, "Contacts"). A removal stays in the block as a
  tombstone (`KeptContact::Removed`) so a replayed record never brings back an
  older card. A new phrase (`contacts::set_recovery_key`) reseals the block's keys
  with the rest and, in the same write in the personal organization
  (`set-recovery-key` is now `Rule::Anywhere`), starts its new epoch sealed to
  the new phrase alone (`AccountRecords::epoch_for`); the server now checks
  only that what the account sealed for itself is resealed, as it cannot read
  which contacts' keys a device reaches. Tests: be-block-client's
  `a_contact_made_on_one_device_is_read_by_another_from_the_accounts_block` and
  `a_new_phrase_leaves_the_old_one_unable_to_read_the_accounts_next_commit`,
  and `a_record_the_server_replays_after_a_removal_does_not_bring_the_contact_back`;
  the rollback and slow-device tests now serve or write an older block.
- **4b2. Links' codes, pins and bootstrapping (done).** The account's block
  also keeps each link's code and kind until it is finished
  (`contacts::KeptLink`, kept before `create-link`) and what it pins per
  organization. `pin-escrow`, `query.links` (`own-link`), `own-sealed`
  (`recovery-key.own`, `set-recovery-key.own`), `create-link.code`,
  `invitation-row.code` and the `pinned_escrows` table and `links.code` column
  are gone; the server checks only that a new phrase reseals what the account
  sealed for itself. Where it differs: a pin made with another write arrives
  as `letter.pinned` (from `create-organization`, an invitation's
  `accept-link`, `create-agent`), taken into the block as records are, and an
  agent keeps its letter. `finish_links` works from the `letter.accepted`
  letters and the codes the block keeps, and drops a link's code once it is
  finished; an invitation's code goes 30 days after it was made, and the block
  keeps at most `MAX_KEPT_LINKS`, the oldest going first. A pin carries when it
  was pinned (`Pinned::made`), and is taken only over an older one, so an old
  pin letter the server sends again is not. `AccountRecords` is opened once per
  connection and identity, and forgotten as the account takes a new phrase.
  Bootstrapping: `Peer::open_session` reads its pin from the block
  (`contacts::pinned`). Tests: be-block-client's
  `a_new_device_opens_a_company_organization_from_the_pin_in_the_accounts_block`
  and `an_older_pin_the_server_sends_again_is_not_taken`.
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

Done (4b): a test where a contact made on one device is a contact on a second
device that reads the account's block, and the server's database holds no
sealed contact record; one where a new phrase leaves the old one unable to
read the block's next commit. (4a's two, a notice under a week old refusing
deletion and one older deleting, and a key change reaching a contact offline
read from the mailbox, are built.) Guides: guides/blocks/keys.md
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
  (guides/blocks/keys.md, "Access logs"), so `remove-member` without a record
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

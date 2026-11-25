# The client: peer, connection and live sessions

Part of the block stack (guides/the_new_block_stack.md). `be-block-client` is
a peer: a local object store, a connection, a refs database and a key book.
The host builds one per workspace (guides/blocks/host.md); `be` uses the same
crate.

## Connection

`Connection` is a `be_wasm_ops::net::Peer` (`Side::Guest`) that implements
`Caller`, so `be_block_protocol::read`, `ask` and `write` and the generated
`be_block_protocol::client::server` functions take `&connection`. It works in
no organization of its own: each call names one (a `Peer` its own,
`Peer::organization`), or none for the account's own, so one connection may
serve any of the account's organizations. Below it, `transport/` has a `Writer`, a `Reader` and a
`spawn` per platform (tokio and tokio-tungstenite natively, the browser's
`WebSocket` in `web.rs`). Natively an `http(s)://` URL takes the server's HTTP
posts (`transport/native/http.rs`), and both go through `HTTPS_PROXY`;
`be_block_client::authorize` adds a `Be3-Gateway-Authorization` header for an
origin (native only: a browser cannot add one to a websocket).
`be_block_client::call_once` makes one call that needs no sign-in as a one-shot
post (`post_once`: natively the HTTP transport, on the web a `fetch` with
credentials), which is how a browser sign-in is begun and polled. Every
connection signs in with its greeting: `Connection::connect` takes
`Credentials` (a token, or `Header` for the request's Authorization header a
proxy injects) and answers the connection and who it signed in as, or the
server's refusal, the connection then closed (guides/blocks/server.md,
"Signing in").

It opens the server's one `events` stream on the first `subscribe()` and pumps
it into one `broadcast` of `be_block_client::Event`. As it attaches, a peer
follows what it hears as it changes (`Peer::follow_topics`: a read with
`follow` of its keys, key work, roots and notification modes, and the account's
contacts, mailbox and storage, each let go of when the server refuses it, as it
does a block's agent); each topic arrives as an `Event` that says only to read
again (`KeysChanged`, `KeyWorkChanged`, `Roots`, `StorageChanged`, ...).
Blocks are read, not announced: `Peer::listing` reads a listing and the
sequence it was read at (`Listed`), `Peer::summary` one block (a `blocks`
query). What changes arrives as trees: `Peer::changed_since(entry,
since, follow)` reads the tree of an entry point (`block-summary.entry`) from
a sequence, following it with `follow`, and `Event::TreeAdvanced` tells when a
followed tree changed, once until it is read to its end
(guides/blocks/server.md, "Listings and following trees"); `Peer::unfollow`
stops. `Event::Roots` says the top, the trash or Shared with me may have
changed. A summary's `changed` orders it against every other of its block: the
refs database keeps the newest (`Refs::put`), and a change this device makes
offline is stamped just past the newest sequence it knows (`Refs::revise`). Key
work changing arrives as `Event::KeyWorkChanged`, on which the host reads it
(`Peer::key_work`) and tends it. A receiver that falls behind re-lists what it wants, reads again
the trees it follows and rejoins its sessions, as after a reconnect.

**Deciding the server is gone.** A socket can stay open with nothing behind it
(a laptop that slept, a proxy that stopped forwarding) and the transport never
says so, so `Connection` decides: see `CONNECT_TIMEOUT`, `PING_INTERVAL` and
`PONG_WAIT`. `ClientError::is_unreachable` covers a timed-out connect and a
disconnect; a key lookup that could not reach the server passes that error on
rather than `Locked`, which means the keys answered and none opens
(guides/blocks/keys.md). The worker alternates reading frames and writing commands and keeps
reading while a write is under way, so neither side's flood starves the other.

Every deadline reads a `be_block_client::Clock` passed to
`Connection::connect`: `Clock::real()` except in tests, which pass
`Clock::manual()` and `advance` it. `Connection::due()` is the clock time the
connection next acts on silence; be-host's `Harness::miss_a_ping` advances to
each `due()` until it gives up. The fault relay (guides/blocks/server.md,
"Tests") supplies the silent server.

## Reading and writing

- `open` reads the block as this device has it newest (`Peer::current`: the
  head of its own commits still in the outbox, from its store, else the head
  commit and its objects in one `ReadContents`, with the session's journal),
  and decodes with `BlockContent::from_stored`; `read_contents` returns
  `Stored` (bytes and the parts they were saved in).
- `save` seals `BlockContent::parts` and publishes against the head it read,
  carrying the commit and the objects (nodes and chunks) that head lacks, so a
  save is one round trip and costs what changed. The outbox keeps a commit's
  full object and reference lists; `send_pending` sends them as changes from
  the expected head's (`held_by`, `references_of`). A merge, or new objects over
  half a frame, upload first (`push`): only what `MissingObjects` says is
  missing, through URLs (below).
- **Changing a block against its head** is `Peer::update(block, content_type,
  make)`, the one read-change-write loop: it reads the block (`current`), asks
  `make` for its new content (`NewContent`: the stored form and its references
  and derived metadata, as `NewContent::of` makes them for a `BlockContent`)
  or nothing, and writes that against the head read. A write refused because
  another landed first is read and made again, up to `UPDATE_ATTEMPTS`, then
  `ClientError::Contended`, so nothing it writes goes over a write `make` did
  not see. Before each write after a refusal it waits a step longer, with
  jitter (`backoff`), on the connection's clock when that is real; on a manual
  clock no real time passes, so it waits for nothing, and tests whose writes
  contend on purpose connect on one. The typed form reads the head's stored form alone, not
  the session's journal past it. `make` answers a future that borrows nothing of it, so an update is
  `Send` whenever that future is; state kept across reads (a type module, what
  was written) lives outside it. `update_content::<C>(block, ops)` is the typed
  form, for content this device decodes: `ops` answers the `LiveEdit`
  operations to apply to the content as read (the prefix registry, the
  account's block). `force_update` is the force form, for content that
  replaces the whole block (a reset to blank, a block's bytes replaced): it
  writes the same content again over whatever head another write left. The CLI's
  edits, a sync pass listing a folder's children and the host's replacing a
  block's bytes all go through these.
- `read_contents` reads many heads at once, asking for up to `CONTENTS_BLOCKS`
  blocks per `ReadContents`, all sent together, so a tree costs one round trip,
  not three per block. The server decides how much of each comes back: the
  first blocks whose summaries fit its answer, and of their heads' objects and
  journals what fits beside them; `read_contents` asks again for the rest of a
  group, and fetches objects left out by URL. Export and `be download` use it.
- `load_first_part` loads a type's first part (`Loads::FirstPart`), and
  `read_span` reads a byte range of a stored form, fetching only the objects
  under it: a blob's payload is read this way.
- `fetch` verifies hashes; `fetch_history` must run before asking history a
  question.

### Objects

Bulk object bytes never travel inside frames (guides/blocks/server.md,
"Objects over S3"). `fetch` and `try_fetch` ask `download-urls`, at most
`MAX_OBJECT_URLS` at a time, and GET each with `http::get`, at most
`MAX_OBJECT_BYTES` and `TRANSFERS` at once (`peer/transfers.rs`); an object
with no URL, or one the store answers 404 for, is missing, and bytes that do
not hash to what was asked are `ClientError::Tampered`. `push` and imports ask
`upload-urls` with each object's length and PUT the bytes of each one it answers a
URL for (none for one the server holds already) with the headers given; an
object the device does not hold (a part reused by its ref) is asked for with no
length, which the server answers none for or refuses as not found, which is
`ClientError::MissingObject`. Uploads go natively and from the web build alike (`crate::http`: in the browser a
`fetch` without credentials). An import uploads a batch's objects before
sending its creates and publishes, since those are sent as soon as they are
made. A transfer that may succeed later (`http::Error::Failed`: unreachable,
5xx) is `ClientError::Disconnected`, which callers retry or keep for later;
any other refusal is `Refused(Storage, ..)`.

## Live sessions

A block being edited is in a session, which each device holds in its type
module's replica (guides/type_modules.md); the peer only relays, submits, seals
and stores for it. `Peer::relay` puts a relay on the connection without waiting
for the answer to the one before, so a pen stroke's presence costs no more round
trips than one message.

The server orders the session (decisions/live_edits_are_ordered_by_the_server.md).
`Connection::join_session(organization, block, tag)` (a `session` read that
follows) names the build of the block's type the connection runs, and answers
the session's membership as of joining (`SessionState`), which
`Event::Membership` entries then follow; the session's events that arrive
before the join is answered are held until it is, so none reaches a subscriber
ahead of it
(guides/blocks/server.md, "Membership"). Of joins of one block on their way at
once (a rejoin asked while the first join is still out), the server answers
the newest last: it takes over what the others held, and only its answer
changes what the connection knows of the session and lets the held events go;
an earlier one answered after it changes nothing. Dropping a join's future
takes it off, so an earlier join still out answers in its place, and once none
is left what was held is told as it came. The session speaks its first
participant's build (`Membership::tag`) and the server refuses submissions from
any other.
`Peer::submit` seals a submission's payload with the block's key and hands it to
the server (an `ordered` send), which numbers it, keeps it in the block's journal and
sends it to every participant, its author too, as `Event::Sequenced`; the answer
comes once it is kept. Every replica applies that one order. A replica keeps
`confirmed` (the order applied) and `visible` (confirmed plus its own edits not
yet numbered). The lease holder seals at pauses (guides/type_modules.md, Peers):
a seal is a publish whose `Write::seals_journal` carries the journal's base, which
trims the server's journal only as the commit wins the head, and every
participant hears which commit holds which sequence number (`Event::Sealed`).
A replica that follows takes the seal as its head by id, and its host fetches the
commit and its content at once (`Peer::try_hold`): the server prunes it once a
later publish replaces it, and what the follower seals offline on it needs it,
to seal and as the base its merge is made against. One fetch runs per block, and
the latest seal heard meanwhile waits for it (`Replicas::fetch_seal`); this
device's own seals are not fetched. A type that loads only its first part
(`Loads::FirstPart`) fetches the commit alone, whose content may be far larger
than what it reads: if the server prunes the seal before its parts were read, an
offline edit on it merges with no base. A fetch that fails is not tried again;
`Peer::reconcile` fetches the base itself once the device is back online.

- **One edit in flight.** A replica's typing waits in its `unsent` queue while an
  edit of its own is unconfirmed, folded by `LiveEdit::absorb_operation`
  (`SeqOp::absorb` for a sequence), stopping before the submission size limit
  (decisions/a_replica_keeps_one_edit_in_flight.md).
- **The lease** only decides who seals and who merges a head moved from
  elsewhere; taking it moves no state, since every replica has the order and the
  last seal's head. Every replica derives the holder from the session's
  membership (`Membership::holder`), and a claim (`Connection::claim`, a host's
  `Event::Claim`) takes it at once (a terminal host takes the block from the
  viewer that opened it).
- **Conflicts.** A merge that conflicts first stashes the holder's unsaved side
  as a commit (`Command::Stash`), so the merge has both sides as parents, and
  reports a `be_block_type_host::Conflict`; be-host keeps it per block
  (`be_host_stack::conflicts`) and shows "See both versions".
- **Incompatible entries.** An entry this build cannot decode marks the session
  incompatible; every replica of the build passes over it alike, so their copies
  agree. A replica whose build is not the session's is diverged and refuses to
  edit or seal.
- **Offline**, there is no server to order: the device numbers submissions in its
  own order over its own journal (`Refs::number_offline`, which numbers an OpId
  once, as the server does), and the replica, which
  holds the made-up session's lease, seals what it orders into commits the outbox
  keeps. Before it rejoins, the host seals it, so the work reaches the session as
  a commit that merges, not as operations.

## The journal

What the order numbered since the last seal is durable in the block's journal,
so a commit can wait for a pause. A journal is a base (a commit, or none for no
content; the last sequence number it holds; the session state as of it, sealed
with the block key) and entries (sequence number, the submission's id and base,
its floor, when the server took it, and the sealed payload); the server keeps it
beside history, never in it (`be_server::journal`), and records the build that
wrote it. A journal starts on the block's head when the first submission
arrives.

- The server numbers and keeps each submission in one transaction
  (`journal::submit`, with `be_block_session::Sequencer`), refusing one of another
  build while the journal holds entries; a submission whose id the journal holds
  is answered and not numbered again.
- A seal trims it: the journal starts from the seal's commit at the sequence it
  holds, keeping what came after. The server's is trimmed by the seal's publish
  itself (`Write::seals_journal`, kept with the commit in the outbox
  (`Pending::journal`, with the build that sealed it) so a drained seal trims too), only as it wins the
  head; `Peer::rebase_journal` trims this device's, and with `trim` the
  server's with no new commit (a publish of the head on itself, which the server
  takes only from the holder; none for a base with no commit, which stays on this
  device). A seal names its build and its run of the order (the replica's
  `restarts`), so the server passes over a stale one (guides/blocks/server.md,
  "Membership"). A base that goes back while nothing after the
  journal's own base was numbered (the last one out, sequence zero) starts the
  order afresh. Entries of another build are dropped, and the journal marked
  `lost` until it moves to another commit, so every device opening the block
  meanwhile is told.
- This device's journal in the refs database copies what it heard
  (`Peer::heard_entry`, as each `Event::Sequenced` arrives), and is trimmed by
  this device's seals, so a replica opened offline opens on the order as this
  device last knew it; offline it also holds what the device numbered itself.
  Every join, first or again, makes it the journal joining answered
  (`Peer::keep_journal`, `replicas::mirror`): with no copy, the device would
  number offline from sequence zero, behind entries its replica opened on, and
  the replica would pass its own entries over as ones it applied (it reports
  an error when one of its pending submissions comes back so). A block whose
  journal holds entries this device numbered offline, their only copy, does
  not join on opening: it opens on that journal, seals, and joins once a drain
  sent the seal (`Replicas::created`), as a block made offline does. A seal made
  offline (`SealedThrough::offline`) counts the device's own order, so on the
  server's journal it is a head to merge, never a seal the journal reaches.
- **A seal on its way.** The holder goes on applying the order while its seal
  waits on the server. The seal's commit records the last sequence it holds
  (`Commit::sealed_through`), and the server trims the journal to it as the
  commit lands. A seal that has not landed yet leaves the journal on the commit
  the seal was made on: a replica opening on the
  seal replays the journal there and takes the seal as its head when the journal
  reaches the seal's last entry (`reaches_seal`, `journal_for`), and is answered as
  the replica's news (`Resume::Sealed`) only when the seal counts the same restarts
  as the journal's base (`SealedThrough::restarts`): a seal of another run of the
  order is merged as a head.
- **Which edits the order holds.** The journal's base names, per origin, the last
  counter the order numbered (`wire::BaseState`), and a device keeps one origin per
  block (`refs::OpenEdits`), so edits it gives again after the trim that dropped
  their entries are found there and not numbered twice. The base names only the
  `BaseState::KEPT_ORIGINS` origins numbered most recently: an accepted limit. A
  replica opening on a full base that no longer names its origin cannot tell
  which of the edits it submitted the order holds, so it sends none of them
  again (those up to `opening.kept-submitted`) and tells each editor it may have
  been sent (`UNCERTAIN_EDIT`); edits it never submitted are sent.
- Joining (`Connection::join_session`) answers the journal as of joining, and
  what the order numbers after reaches the connection as `Event::Sequenced`, so a
  replica opening on the journal's base needs nothing from another client.
  `read_contents` answers each block's journal (`BlockRead::journal`, opened;
  offline the device's); a reader applies it with `Modules::readable`, which
  replays it through the type when its tag is the module's and its base is the
  head, and reads the head alone otherwise. Export, `be download` and the
  tracker's reads go through it; `be sync` reads heads alone, since it records
  which head it synced and what it writes to disk must be that head.
- A replica of another build opens on the head alone, following the order from
  its last number, and reports the edits lost (guides/blocks/host.md, Undo); its
  holder starts the journal over.

## Signed logs

A block's signed log (guides/blocks/server.md, "Signed logs"; `peer/log.rs`)
is read and appended a page at a time, and nothing the server serves of it is
taken unproven.

- **Appending.** `Peer::append(block, app, payload)` seals a random
  32-byte salt and the payload under the block's key, signs the entry's id
  and the hash of the salt and the app's bytes framed by its id
  (`be_block_protocol::log_signed` over `Said::hash`, under `sign_authored`,
  so the signature binds the block and the app, verifies wherever the
  plaintext and salt go, and confirms no guessed payload to whoever lacks the
  key), and keeps it in the outbox (`refs::Op::Append`). Opening strips the
  salt (`LoggedEntry::opened`, a `Said`; `LoggedEntry::bytes`); a payload that
  opens to less than a salt is `LoggedEntry::malformed`. The leaf
  commits to the sealed bytes, so after a redaction who wrote an entry is the
  log's word, its signature no longer checkable. Online, with nothing of the block waiting ahead of it,
  it goes at once with any of the block's entries that wait
  (`send_appends`, batched), answering `Appended::Logged` with its place and
  arrival; otherwise `Appended::Kept`, and the drain sends it in order
  (`Drained::Appended`). A log's entries wait only behind the block's
  creation, never behind its content: a diverged publish holds none back. One
  the server refuses leaves the outbox (`Drained::NotAppended`,
  `Appended::Refused`) and is kept, with why, until the app lets it go
  (`refused_entries`, `let_go_refused`; `Table::NotAppended`), so what the app
  shows of it outlives the editor and the device starting again. A local
  account numbers and stamps entries on the device (`append_locally`),
  keeping its tree's right edge (`KeptLogHead::frontier`).
- **Reading.** `Peer::read_log(block, from, limit)` reads a page and follows
  the log. The page is taken only when its range proof ties its entries to the
  head the server names (`merkle::range_root`), every payload matches its
  hash, and a consistency proof shows that head extends the one this device
  kept (`merkle::consistent`); then the head and the page are kept in the refs
  database (`Refs::keep_entries`; `KeptLogHead`, and entries by block and
  place), and opened, the block's keys asked for once if one is missing. A
  page anywhere in a log, the first of a million-entry chat's or one a search
  hit points into, costs that page and O(log n) hashes (`LogRead::proof_hashes`).
  Offline it answers what was kept, from `from` up to the first entry it
  lacks. `Peer::logged` takes an `Event::Appended`: at once when it extends
  the kept head, else by reading from the kept head. One told late, after a
  read moved the kept head past it (a page from the log's start, an export),
  is answered all the same, as proved against the kept head, never the head
  the push names alone: those it holds as held, the rest read again
  (`read_log`). Whoever follows the log drops those it holds by their place.
  `Peer::redacted` takes an `Event::Redacted`: the kept entry loses its
  payload (its leaf stays, so the log still proves it) and comes back as it
  is now.
- **Cut and forked logs.** A head the server serves that does not extend the
  kept one (shorter, or with other entries) is `ClientError::LogForked`, and
  nothing of that answer is kept; so is a push of a head no longer than the
  kept one that differs from what is held. A page that names an id twice, or
  whose arrivals go back (within it or against the entries kept beside it), is
  `Tampered`. The kept head moves only by compare-and-set
  (`Refs::keep_entries`): a page checked against a head that a push or another
  read moved meanwhile is read again against the head kept now, so a head is
  never replaced by an older one or a sibling; one that moved to the page's
  own head takes the page as it is. A push told late is checked against its
  own proof first: one that does not prove is `Tampered`, not a fork. `forget` drops a log's entries
  but keeps its head.
- **Exports.** `Peer::export_log` reads a block's whole log, opened, as an
  export carries it (`ExportedEntry`, `transfer::SavedBlock::log`), each entry
  naming the block it was written in, and `Peer::verify_exported` checks an
  entry against that block and its salt, wherever the file was imported, with
  no key of the block. A redacted entry, or one this device cannot open, is
  carried without what it said, and verifies as nothing (`None`). Offline it
  carries the run from the log's start this device kept. An export proves
  each entry's author, app and block, not that the file holds the whole log
  or its order: it carries no head or proof, so whoever edits the file can
  drop entries or reorder them unseen.
- **Imports.** An import keeps a log's entries as read-only history
  (`transfer::renumber` keeps them; a duplicate starts an empty log): each
  goes in the new block's outbox behind its creation (`peer/import.rs`), its
  payload the entry as it was written (`ExportedEntry::sealed`, told apart
  from what an author says by its first byte), so it reads back as
  `LoggedEntry::imported` and still verifies against its own block. The
  importer signs each with its place in the new log and the history's length
  (`ExportedEntry::sealed`), and its verdict (`Peer::verify_imported`) is
  `Bad` unless it landed at that place, within that length, appended by the
  block's creator: no member can replay another block's signed entries into
  this one, and the entry alone decides it, with nothing read, the same on
  every device. No verdict rests on the
  file's arrival (`verify_exported` passes none). New entries append after
  it. A file carries a block's own entries alone (an export of the new block
  leaves its history out), and `transfer::decode` refuses an entry written in
  another block than the one it is saved with.
- **Redactions.** The server tells a log's followers of each redaction
  (`event.redacted`). `Peer::redacted` takes the server's word for it, as the
  server holds the payloads and may drop any anyway; the entry's leaf stays,
  so the log still proves the entry and who wrote it. The server numbers a
  log's redactions (`log-page.redactions`), and each read names those since
  the count this device caught up with (`redactions-after`,
  `Refs::log_redactions`), told again as `Event::Redacted`; as it connects a
  device asks of every log it read at once (`catch_up_redactions`,
  `query.log-redactions`, a few hundred logs a read), so one
  pushed while it was away still reaches it.
- **Who wrote an entry.** `Peer::verify_entry(block, app, ..., said, signature)`
  is `verify_authored` over what the app said and its salt, with the entry's arrival,
  which only the server sets, so a contact's replaced key that
  signed an entry the server took after this device learned of the new key
  answers `Replaced` (guides/blocks/keys.md, "Verifying").

## The refs database

A `Peer` keeps what it learns in `RefsDb` (`be_block_client::refs`, SQLite on
every platform, in the origin private file system on the web), keyed by server,
account and organization: local and last-known remote heads, the newest head
whose content it has read whole, raw `BlockSummary`s (metadata stays sealed),
the pages of signed logs it read and each log's last verified head,
key rows and wraps, verified contacts, every contact's name and the escrow chain, the workspace list, the
outbox, the unsent area, each block's journal and the blocks the search indexer let
go (`Peer::let_go`). A sealed commit and its local
head are written in one transaction (`Refs::push_publish`), and so is a publish
settling: its outbox entries leave as the remote and local heads move on
(`Refs::settle`), so no reader sees an empty outbox beside the old heads. The
remote head never moves back to one it descends from (`Peer::behind`): the
server's head only moves on, so such a head was heard of late, from a summary
read before this device published and answered after, or from a merge settling
a divergence a later one already settled. A server restored from a backup, whose
head is older, is caught up by the next publish: rejected with the server's
real head, the device merges against it (`Peer::reconcile`) and the published
merge moves the remote head on. A record that no
longer decodes is skipped on load. Records are postcard, which is not
self-describing: a field marked `skip_serializing_if` is written as nothing and
never decodes again, so a refs type never uses it, and `#[serde(default)]` does
nothing for a field missing from an older record.

`Peer::list(listing)` reads any `Listing` (`Peer::listing` with the sequence
it was read at; `children` is its only shorthand, and `summary` reads one
block): every page of a long one (`listed.next`, in the order of
block ids, a `References` listing too), and the summaries
of each page's `rest` with `read-summaries`, `MAX_SUMMARIES_ASKED` at a time,
keeping each block once and leaving out a summary read later that the listing
would no longer hold (`still_in`). A block's references past
`MAX_REFERENCES` are refused before anything is sealed or put in the outbox
(`bounded`). The refs database keeps one summary per block, the larger `changed`
winning, and a listing is the ids it held: a copy kept per listing went stale offline (a listing fetched
before another device's seal, or the copy a parent's listing held). Offline, a
listing answers as the summaries place its blocks now:

- a block's `References` are what the content it kept references
  (`Peer::references_here`), so a seal or an offline commit after the listing
  was fetched shows, or the listing as fetched when it kept none;
- its `Backrefs` are the listing as fetched;
- a block's `Children`, and the `Deleted` blocks, are those the listing held
  that their summaries still put there, then those whose summaries put them
  there and changed after the sequence the server answered the listing at
  (`Refs::listed`, kept with the listing). So a block made online, moved in,
  or pushed since the listing was fetched shows, and one moved away does not.

A summary is dropped only with the blocks `forget` drops, so one the server no
longer lists stays until then. No summary names what its block references:
`Peer::content_references` reads it from the block's newest commit this device
has (one in the outbox first, else the summary's head), with its parts'
(`references_of`), or from its metadata for a block with no content yet, in
the content's order. A local account answers `References` and `Backrefs` from
it too, having no server that keeps references.

`Peer::cached` builds a peer from it with no network, and `Peer::attach` hands
it a signed-in connection later (after every reconnect), which opens the
organization and refreshes keys and contacts. While disconnected, `summary`,
listings and `read_contents` answer from the cache, and `open` reads the local
head first while it has unpublished commits.

A summary's head is the server's newest, which this device may never have read
(another device moved it). Offline, content is read at `Peer::held_head`: the
first whose objects the store holds whole of the local head, the server's, and
the head last read whole (`Peer::read_whole`, which `read_contents` and a
replica's whole load record). A block with none has nothing to read offline.

A push notification (guides/blocks/server.md, "Notifications") opens with
`be_lib_web_push::open` (the device's own RFC 8291 keys; a browser does this
itself) to a `be_block_protocol::PushMessage`: organization, block, the sender
the server vouches for, then the text sealed under the block's data key with
the organization, block, sender and send time. `Peer::open_notification` opens
that offline from the cached key rows, with no block load and no type module,
refusing one sealed about another organization, block or sender, or older than
`MAX_NOTIFICATION_AGE_MILLISECONDS`; `Peer::contact_name` and `Peer::kept_name`
name its sender and block from the refs database alone, for a device that opens
a push with the app closed (`be_host_stack::device_push`).

## Saving and the outbox

A save is a `be_block_client::Write` in two steps: `Peer::seal` writes the
commit, records it in the outbox and moves the local head; `publish_sealed`
sends it. `Peer::write` does both.

- **Offline**, a failed publish stays in the outbox (`Peer::write` answers
  `Saved::Kept`), as do creates (`refs::Op::Create`), moves (`Op::Move`),
  defines on a block with no content (`Op::Define`), holds of objects for a
  block (`Op::Hold`, from `Peer::hold`, which version control puts on its
  snapshots) and entries for a block's signed log (`Op::Append`, "Signed
  logs"); creates, moves and defines rewrite the cached summary so listings show
  them, and later ones for a block queue behind earlier ones. `keep_creation`,
  `keep_definition` and `keep_move` put them there online too (the host's
  path); a publish waits behind a creation, move or define of its block, and
  behind a commit of it sealed earlier that it does not build on (a definition
  `keep_definition` sealed on the head it went on too): sent first, it would
  leave that commit behind, so the drain sends both in order and merges them
  (`Drained::Diverged`). A publish that waits answers `Saved::Kept`, never a
  head the server has. Every
  summary the server gives is laid over with what the outbox has yet to send of
  its block, a waiting move's parent and a waiting define's metadata, as it is
  kept (`RefsDb::set_listing`, which `Peer::note` and listings keep it
  through) and where it is returned (`Peer::summary_with_outbox`), and a server
  listing with what the outbox adds or moves (`Peer::with_outbox`): an answer
  the server gave before those reached it would otherwise undo them here. A
  listing is laid over too with each block whose creation or move the server
  took while it was out (`Peer::sent_since`, marked before it is asked for), as
  this device now has it: calls on one connection are answered in order, so
  the answer was read before the server took that write. One the server
  refused was never taken, and the answer stands for its block.
  Each goes up as one write: a creation as `[create-block, put-keys(rows)]`
  (`Creation::ops`), a move as `[move, put-keys(rows)]` with the keys
  `Peer::rescope` made for its new scope (none within one), and a define as a
  `publish` with no commit, expecting no head. Sending a move carries its
  databases' prefixes along (`Moving::from`).
  `Peer::history` answers offline with the history the device holds, walked
  from the server's head as last heard (`history_held`): it can hold commits
  the server has since pruned, where online it lists only what the server keeps.
  Sharing, access, members, invites, agents, contacts and pairing need the
  server.
- **A local account** (`Peer::local`; guides/hosting.md, Local accounts and the default server) has no server:
  its refs scope has an empty server, and the session row is made on the device
  the first time (role editor, kind personal, no escrows). It runs every change
  through the offline paths above, then settles the outbox on the device as a
  server would answer it (`settle_locally`, which is its `drain`): creations,
  moves, definitions and holds are final as they are kept, and a publish is the
  block's head when it goes on the head the block has (`publish_locally`, the
  server's compare-and-swap), else `Saved::Rejected`, or `Drained::Diverged` to
  merge. A settled commit or hold grows the block's `size` by the bytes of the
  objects it did not hold yet, and the `tree_size` of it and every block above
  it, and a settled move carries its `tree_size` along (`Refs::grow`,
  `carry_tree`), as a server counts them. Listings answer from every summary it
  holds (`Refs::local_listing`),
  history from its store (`history_held`), and what needs a server is refused as
  offline. be-host-stack runs it when its `Config` says `local` (set outright,
  never read from an empty `server_url`): the line never goes up (`Line::Local`), the stack starts
  as arrived (`arrive_locally`), and its status says `local`, which the host
  shows as saved rather than offline.
- **Links** (`contacts.rs`): `make_link` and `invite` make a contact link or an
  invitation, and `open_link` and `accept_link` read and accept the kind the
  caller asks consent for (`Expected`), refusing the other, and answer the
  `Link` with its owner's card; accepting an invitation pins the
  escrow key it carries. Agents are members: `Peer::members` lists them with
  their creator, and `Peer::remove_member` removes one.
- **`Peer::drain`** publishes the outbox in order once attached and reports a
  `Drained` per block. A rejected publish whose remote head already contains
  the commit is settled, not diverged; an entry is settled once a published
  head descends from it. `Peer::reconcile` finishes a diverged block, answering
  `Reconciled::Merge` when the replica host has to merge. A commit sealed
  offline under a key the logs now show outdated is sealed again under a new
  epoch and sent (`Drained::Resealed`), or set aside (guides/blocks/keys.md,
  "Outdated epochs").
- **The unsent area.** A block whose access is refused (on a publish, a create,
  `summary`, or a drain refused `PermissionDenied`/`BlockNotFound` after
  re-reading the summary) moves its entries to `Peer::unsent`;
  `restore_unsent` brings them back, `discard_unsent` deletes them, and
  `UnsentBlock::offerable` says the summary allows editing again.
  A create refused for any other reason (over a quota, too large) is set aside
  the same way, so nothing waits behind it, with the code it was refused with
  (`UnsentBlock::refused`, `Drained::SetAside`): the host shows it, an
  organization's lost-access policy never deletes it, and cached listings leave
  it out. What is made in it is set aside with its refusal, and restoring it
  restores them. A publish refused as a change the server will never take
  (`InvalidRequest`, `ObjectTooLarge`, `CommitAhead`) is set aside the same way rather than
  sent again every round. A refused move or define puts back the summary the server has
  at once, with what still waits of the block laid over it (`unchanged`,
  `Drained::Refused` with `restored`).
- `Peer::pending` counts outbox operations, local commits, their blocks and
  unsent blocks.

## Tests

The round trip through a real server is tested in
`crates/be-block-client/src/tests/`, with `Harness` in `src/tests.rs`.

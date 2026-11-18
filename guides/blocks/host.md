# How the host holds content

Part of the block stack (guides/the_new_block_stack.md). The host is the peer,
not the app: an app never sees a key, the connection or the server. It is
handed content and graph answers and hands back operations and graph commands
(guides/blocks/apps.md). The code is `crates/be-host-stack`.

## The peer and its registry

The host's peer names no content type. Every type runs in a type module
(guides/type_modules.md): an app's own, named by its manifest's `block_types`,
staged beside the app as `NAME.wasm` with `NAME.sha256` (the
tag of its sessions), one instance per app; and the host's own types
(`be_sdk_type_host`'s `be_block_types!`: settings, repository, checkout), run
in process as its `BlockTypes`.

A type's name, icon, color, child edits and press come from the declaring
manifest (or `be_host_stack::host_block_types`), held by discovery
(`editors/app/discovery.rs`), so a type is known before its module starts; an
app that declares a taken type or edits an undeclared one is refused. The
registry (`be_block_type_host::modules`, sources listed in
`be-host-stack/src/types.rs`) starts an app's module only once a block of its
types needs it, so `be_host_stack::kind_of` is `None` until then and the worker
asks `Replicas::kind`, which starts the module. Merging, describing and
validating are requests to the module (`Replicas::merge`, by parts for a type
that merges by parts, guides/type_modules.md "Merging by parts"; `describe`). A block
whose type no module has has no content in the host (`be_host_stack::provided`).

## Blocks saved by another build

Every save stamps the derived `SCHEMA` metadata with `schema_fingerprint::<C>()`
(a hash of `BlockContent::schema()`: the model's `Kind::describe`, or `blob N`
from `BlobKind::SCHEMA`). `be_host_stack::staleness` calls an unfrozen block (every block, for now)
stale when its replica could not read it (`Status::unreadable`) or its stamp is
another schema's; no stamp is not stale. A stale block's frame shows
`StaleNotice` (`surfaces::StaleBlock`) with Reset (`be_host_stack::reset`, the
type's default content), Delete, and Try anyway (`try_anyway`, for the
session). A block the host holds itself is reset without asking, with a notice.

The host keeps `be-objects/` and the refs database across builds.

## The worker and its replicas

`be-host-stack/src/worker.rs` is the peer's loop. `be_block_type_host::Replicas`
(shared with `be`; be-host's `Host` is `replicas.rs`, where reports land in
`Shared`) is the host half of every open block's replica: it joins the session,
reads the block's head (which lists it, so its followed tree tells it the heads other
connections publish), constructs the `replica` with the head, answers its calls
with the real primitives (`Peer::seal` then `publish_sealed`, `Peer::stash`,
resume, the lease, relays sealed with the block key, `sleep-until`) and keeps
what it reports (`report-status`, which carries the undo state too, `described`, `conflict`).

A module runs on its own thread natively, in a web worker on the web
(`WebGuest`), and in process in unit tests (`InProcess`); the worker never
waits on one. A module that stops (a panic, or a step past `STEP_TIMEOUT`) is
restarted and its replicas reopened from their last heads with this device's
journal on them (Opening again, below), editors attached and messages since
the last seal replayed. `Replicas::forwarded` counts each attached editor's
operations per block, kept across reopens until it detaches and less any a
replica that ended never took; attaching an editor again tells the replica that
count less those about to be given again, and its snapshot waits for `resent`,
after them (guides/type_modules.md, Editors are endpoints). `native.rs` and `web.rs` run the loop
over a `FileStore` (on the web in the origin private file system).

Nothing in the worker runs on a timer: it waits on commands and the
connection's broadcast, with one deadline for the autosave a dirty session is
owed. `status().wakes` counts wakes, and a test asserts an idle peer never
wakes. The tasks beside the loop run only while it waits, so a pass whose wait was
over before it began yields once; a wait that fails reports how many such
passes came in a row.

### What the page reads: `Seen`

The worker's state is `Shared` (`worker.rs`); the part of it the host's synchronous calls in
`lib.rs` read (the graph mirror, open blocks' content, histories and descriptions, version
statuses, status counts, contacts, pairing, conflicts, search answers, editors' frames) is its
`seen: Seen` (`seen.rs`), and `Shared` derefs to it. The host never locks `Shared`: the
worker hands `Seen` over in bytes (postcard), the whole of it first and then each time what
changed since the last (`seen::Update`, `diff.rs`), onto a lock-free stack (`Handover`) the
host takes every update from at once (`take_updates`), and applies in order to its copy.
Past 64 updates or 4 MiB the host has not taken (a hidden tab runs no frames), the worker
swaps its whole `Seen` in for them (`Handover::replace`). Neither side waits on the other, as the web's page must not (decisions/the_web_page_never_waits.md). The large
parts are `Part`s, an `Arc` the worker's last handed-over copy (`Shared::handed`) shares until
the worker writes it. The maps in them are persistent and ordered (`imbl::OrdMap`), and the
graph's nodes are `Arc<Node>`, so that write copies the path to what it changes, not the
part, and what changed is found by walking the two versions' trees, skipping every subtree
they still share: a head a replica reports, or one another device published, copies and
hands over one node of the graph however large it is (the graph's
`a_head_change_copies_one_node_not_the_graph_a_seen_shares` and
`a_head_change_hands_over_one_node_not_the_graph`, and the stack's
`a_keystroke_copies_no_more_of_a_large_graph_than_of_a_small_one`). Anything else goes whole
when it is not equal to what was handed over; what only the worker reads (the graph's moved
nodes and logs) stays behind. A debug build's update carries the whole `Seen` now and then
(each time in be-host-stack's tests), which the host asserts its copy equals, so a field the
diff misses cannot drift apart unnoticed. A write that would change nothing must not be made:
reaching a `Part` mutably copies it, so hot paths ask first (`Graph::reports`, `refers`,
`derives`, `wanted`) and write only on a change.

The worker publishes at the end of each pass and before the host hears anything from it:
change notices (`changes::post`), the answers to the host's replies (`seen::hold_answer`,
a flush's too) and the notices shown to the person (`seen::notice`) are held in the handover (`Handover::told`) and go in the next update that hands
over the worker's `Seen`, whoever makes it: the one a task publishes once the worker yields,
which then wakes the host, an edit the worker makes itself (`here_mut`), or a test's
(`wait_until`, `with_shared_mut`), so the page never reads what the worker did without
hearing of it. The host takes the updates once a frame, as be-host's
frame starts (`take_updates_in_frame` in `frame.rs`), so every read in a frame, be-host's and
the app runtimes', reads one snapshot; then its watchers hear the changes and the replies'
callbacks run with their answers, reading what the worker did before answering. An editor's
frames are messages to it, not state: they go as soon as they are handed over, not at the
next frame (`take_editor_frames` decodes what came since into `Stack::fetched`, which the next
take applies). So within a frame an editor can hold a frame whose effects (a new child, a
moved head) its host's reads don't show until the next frame's take: frames and reads agree
only from the next frame. Answers that land after the stack stopped are lost with it. A test that runs
no frames has a thread take each update as it lands under `testing` (`start`), until a frame takes them;
that thread tells the watchers only after it applied the update, so a test that runs passes
takes the updates as each starts (be-host-runtime's `next_screens` does under `cfg(test)`). What answers
outside a frame awaits its reply taking updates as they land (`taking_updates`): a hidden
tab runs no frames, and a push the service worker asks the page to open must still be
answered (`web_push.rs`). What must block on the worker natively (a flush as the window
closes, a push opened by the running stack) takes updates as it waits (`wait_for_answer`);
the web's page never blocks. `wait_until`, and a
test's change to `Shared`, publish and take too.

What the host changes itself is an `Edit` (`create`, `define`, `set_parent`, `want`, an
import, revision previews, comparing conflicts, taking an editor's frames): applied at once
to the host's own view and sent as `Command::Edit`, numbered, which the worker applies as
it takes the command, in the order numbered (`edit` numbers and sends under one lock). Until
a `Seen` says the worker applied it (`edits_applied`), the host lays it over each `Seen` it
takes, quietly (`changes::quietly`): the host heard of it when it made it.
Everything else the host asks for is a command;
`Seen::done` against the commands sent is what is pending. A command is data: the host sends
it as its bytes (`worker::CommandPost`, postcard) and the worker decodes it as it takes it
(`worker::Commands`), so nothing in one is a pointer or a callback. A reply is a number
(`replies::Reply`): the host keeps what it calls back in its stack (`replies::expect`,
`Stack::waiting`), and the worker answers with the answer's bytes in an update, or drops it
unanswered, which drops what waits on it; what still waits when the stack stops is dropped. A
`Seen` encodes the same way (`Seen::encode`). What only the worker's `Peer`
can say is worked out into `Seen` too: offline, which blocks are out of reach
(`Seen::out_of_reach`, worked out again only for the nodes that moved and the blocks the worker let go of, `Shared::refresh_reach`), so the host holds neither
the `Peer` nor the connection; stopping drops a oneshot (`Inbox::stopped`) on which the
worker closes its connection. The modules' held clock and the offline choices
(`offline_choice`) are handed to a stack as it starts (`worker::Start`), then as
`Command::HoldClock` and `Command::OfflineChoices`. The page reads which block types the
worker's modules know from `Seen::kinds` (`be_host_stack::is_known`, `kind_of`), never from
be-block-type-host's registry, which only the worker reads; what the worker shows through
be-host-base's notices goes in an update too (`seen::notice`), as answers do.

On the web the page and the worker each hold their own ends of these channels and their own
`Handover`, and a `MessagePort` joins them (`web.rs`): the page posts its commands, nudges and
how much it took (`ToWorker`), and the worker posts each update, marked when it goes in place
of those before (`ToPage`, which the page's handover takes in with `seen::put_in_place`), and
a wake asked for with none (`Handover::wake_page`). The worker counts `held` as what it posted
that the page has not said it took (`Handover::acknowledge`). The page dropping its port ends
the stack, as dropping its ends of the channels does natively. Natively they are one process's
channels and one `Handover`.

Tests wait on the stack with `be_host_stack::wait_until(what, read)`, which
fails at the caller, naming `what` ("opened the note"), after `WAIT_LIMIT`
without the stack saying anything (or `WAIT_CAP` in all) instead of waiting forever: tests share one stack in turn, so one endless
wait would hang the whole target. A busy machine slows the stack without
silencing it, so the limit counts silence, not time. A stack on a driven
clock (`Clock::driven`, which be-host-testing's harness uses) also has its
waits move the clock on to the stack's next reconnect, and all the stack's
time with it, so a failed connect never strands on a clock nobody moves. A
test that moves its clock itself uses `Clock::manual`, which no wait moves.
Commit times and an embedded server's history follow `be_block_commit::now_milliseconds`, which a
test pins with `pin_now` to step history a minute at a time (be-host pins it to
its wall clock while that is pinned, so `drive pause` moves it).

The host refuses an `Operate`, a replacing `write-content` and graph commands
for a block the account may only read (`Instances::editable` asks
`be_host_stack::access`); it does not rely on the app's `editable()`.
`be_host_stack::access` is the one answer to whether a block's content may be
edited now: the mirror's access, but at most `View` while the block is marked
(`Seen::marked`, below, "discarded"). Editors are attached with it and an app
is opened with it (`OpenBlock::editable`) and told when it changes
(`EditorEvent::SetEditable`).

## Connections

`accounts.rs` is the account client: it connects to `{server}/api/be`
(be-server accepts any path; the suffix matters only behind a routing proxy),
registers, signs in and out, lists and creates organizations, and handles
invites. `link.rs` holds one connection per signed-in account and server until
it closes; sign-in hands it over (`link::adopt`) and others borrow it
(`link::signed_in`), reconnecting only once it closed. A connection keeps the
clock it was opened on, so a worker on a test's `Clock::manual()` opens its
own.

The server keeps one open organization per connection, so only the worker
opens one, after the last worker stopped (`Link::open_organization`). Opening
it again drops the connection's listings, so anything needing the
open organization's escrow key (inviting, creating an agent) goes through the
worker's `Peer`.

The refs database is one per device, `be-refs.sqlite3` beside `be-objects/`
(`be_host_stack::refs_in`, `/be-host` on the web). The worker builds its `Peer`
once per workspace, with `Peer::cached` when the database has a session for
that server, account and workspace (so the graph and open blocks come up with
no network), and hands every later connection to it with `attach`.

## Disconnects, offline and stopping

`flush()` waits until the server ordered every edit an open replica in a live
session submitted (`Replicas::ordering`), seals, and waits until every open
block is kept: in the server's journal, this device's, or a local commit. It
waits for the server no longer than `FLUSH_TIMEOUT`, so closing a host whose
server stopped answering does not hang; what that server never ordered is
still kept on the device (Opening again, below). `stop()` flushes first,
so organization changes do too, because closing the connection would lose an
edit on its way; `crash` does not. What a flush kept is sent as the outbox
drains. Commands that seal the open blocks first (a version snapshot, the
bookmark a restore keeps) wait the same way in the worker (`after_order`), and
until the modules took every edit sent before them; then they pass the same
waits as every other command (`admit`: for their uploads, then their turn
behind the server lane). One released from those waits while edits are again
unordered goes back to `after_order`, since its seal must come after them.

A dropped connection (`Connection::closed`, not the broadcast, which outlives
the socket) leaves content on screen; the worker backs off (`reconnect_delay`,
cut short by `retry_now`), reconnects, reloads the graph and rejoins every open
block. Meanwhile it keeps serving from the same `Peer` (the loop's `Line`:
`Up` or `Down`), and every open replica is handed a made-up session whose lease
it holds (`Replicas::disconnected`), in which this device numbers what it
submits, and each is asked to seal at once, locally, since reconnecting reopens
it on the server's journal. `Replicas::reconnect` asks the same of every replica that holds
unsealed work (one opened offline that replayed this device's journal, one
opened as the outbox drained), and a join's answer waits (`Replicas::settling`) while a seal of the replica is asked, on its way,
or answered and not yet reported, since its head is moving past the reported
one, and while a submission of it is unanswered (`Open::submitting`), and
when, after it sealed once, the order holds journaled edits no seal holds and
no seal would take them (`rejoin_held`: it diverged, or its seal backs off or
cannot merge), until a status says one would. A block closed or reopened
meanwhile lets go of that join instead (`Replicas::let_go_if_held`): its
replica ends in its made-up session, sealing there as it closes, and one
reopened opens afresh. A replica that diverged, or whose seal cannot merge,
seals nothing as it closes: its journaled edits are then only in this device's
journal, which a replica opened online passes over when the server has a
journal of the block. What the
order numbered while the join was on its way reaches the replica it opens
before its edits are given again, so one the order took meanwhile is not
submitted twice: the join's journal may predate it, and each opening numbers
its submissions from a fresh counter. `Connection::is_closed` reads true as soon as `close` is called, a moment
before `Connection::closed` says so, so be-host's link never hands a new stack
the connection a stopping stack closed.

- **Opening again.** A replica opened again (on joining, rejoining, a module
  restart, or when the host fell behind what the server sent) starts where
  `journal_for` says: on the server's journal, at its base, when there is one
  (the order goes on from there; the host then tells it of heads that landed
  since, and its holder merges them in), or on a seal made on that base that the
  journal reaches (`opening.sealed-head`): the head, or for the server's journal
  a seal the head descends from through seals, as when the server lost a seal's
  rebase and the outbox published it and those sealed offline on it since,
  whose entries the head holds and merging it into the base would count again
  (the walk stops at a merge, and is bounded by run, by the base's number, and
  by depth; plans/simpler_server_api.md step 1 removes it);
  offline, on this device's journal when
  it is on the head, entries included, `journal-kept` when this device numbered
  them. This device's journal keeps the server's as joining answered it. A
  server with no journal of the block starts one at its head: online, this
  device's journal is passed over then, unless it holds what only this device
  numbered, and a replica joining as the lease holder first starts the server's
  journal from this device's when that is on the server's head
  (`started_from_mirror`), so the session state its last seal kept, which an
  editor shown that content numbers its next edit by, goes on. The
  replica is given again the local edits its module took that neither the head
  nor the order holds (`Open::replay`), with the messages tentative gestures
  hold where they arrived, which no cut or keeping on the device takes
  (guides/type_modules.md, Tentative gestures): every status cuts `replay` at
  `status.sealed-taken` (what its head holds, a replacement on its way not
  included), and a reopen at `status.journaled-taken` too. The replica keeps its
  `origin` (`Open::origin`) and is told how many local edits it is no longer
  given (`Open::given_up`, `opening.taken-before`), so it numbers what it is given
  again as before and skips what the order holds already: an edit whose answer
  was lost is never made twice. The same holds across a stop or crash: the host
  keeps each open block's origin, `given_up`, the last edit it submitted online
  (`Open::submitted_through`), the editors' frames in `replay`, and each
  operation submitted online that holds some of them, with its counter, `through`
  and payload (`Open::submissions`) (`refs::OpenEdits`, sealed, written as they
  change by `Replicas::keep_edits`, and before any submission that holds an edit
  past the `submitted_through` last written, or names a submission not kept yet
  (`Open::unkept_submissions`), goes out; a write that fails fails the
  submission, except for a block's key not on the device yet, when the
  submission waits and is written ahead when it goes again), and a replica
  opened later is given them under that origin.

  Once joined, a replica submits the edits a kept submission holds as exactly
  that submission (`opening.kept-submissions`, the replica's
  `kept_submissions`): the same id and payload, so if the first copy reaches the
  server too, late, from a connection of the process that crashed, the server's
  id check (`journal::submit`) answers it held rather than number the edit
  twice. It waits until every edit the kept submission holds is given again, so
  their announcements go with it, and goes under its id only if they encode to
  its payload again; otherwise they go as a new submission. One the order
  numbers meanwhile takes the edits it holds out of what waits. When the order
  restarts, the submissions made before it are passed over: the replica drops
  its kept submissions as it carries its edits over, and reports the restart
  before it submits them again (`Replica::flush`), so the host drops those it
  keeps of before the restart (`Replicas::take_status`) and keeps the new ones.
  It drops them only on a restart past the most it has seen (`Open::restarts`,
  which the edits are kept under too): a replica reports none before it loads.
  Every opening, after a stop or in the same process, is told that count as
  the restart its edits and kept submissions were made under
  (`opening.kept-restart`); a replica opened on an order that restarted since
  drops those edits as stale and the kept submissions with them.
  A reload or a stash names as its `through` only what the order holds already
  (`Replica::entry_through`), never edits still unsent, which a replica opened
  again would take as ordered, except those a replacement from elsewhere
  dropped (`Replica::replaced_through`), which are gone and must not be sent
  onto the content that replaced them.

  Kept edits that do not open while the block's own key is not on the
  device stay as they are, and the block waits locked until keys arrive
  (`Replicas::lock`), so no replica writes over them. Whether the block's key
  is here is asked read only (`Peer::holds_key`); offline that cannot be told,
  and the block is neither locked nor opened until the host reconnects. Ones that do not open
  otherwise (unreadable, or under a key that has not come though the block's
  has, such as a key generation that has not arrived yet: those are set aside,
  not waited for) are set aside (`refs::SetAside`, never sent) with an error
  naming how many, and the block opens under a fresh origin. Once keys arrive,
  set-aside edits that read are told so, and that they were not sent
  (`Replicas::read_set_aside`), and left a tombstone (their count). A block
  keeps its newest `SetAside::KEPT`, and forgetting the block drops them; a
  block whose kept edits hold any is not forgotten, until this account lost
  access to it and its unsent edits are discarded (`Peer::discard_unsent`,
  which in the same step drops its kept edits, its journal and the commits it
  sealed still in the outbox, and puts its local head back on the server's;
  the unsent list shows how many kept edits it holds,
  `UnsentBlock::kept_edits`). Its open replica closes
  (`Replicas::stop_keeping`), and one opened next starts under a fresh origin.
  Until the discard no replica that joined shows the edits set aside, though the
  local head stays on them: one that lost edit access follows the server's
  order, and a session of viewers has no holder to merge them in, so they show
  only among the unsent edits and as not sent to its editors
  (guides/blocks/apps.md, "Sending"). What it takes meanwhile (an editor
  attached before the access changed, or the host's own edits) the server
  refuses (`PermissionDenied`): the host answers the submission `refused`,
  drops what it announces (`Peer::submitted_refused`), and the replica gives
  the edit up, shown as not sent until its editor lets it go.
  The discard marks the block (`refs::Dropped`), until a join's `may-edit`
  says this account may edit it again (`Peer::join_session`): a listing that
  says so may be older than the access that lost it, so it does not count,
  and forgetting the block keeps the mark, as its replica may still be open.
  While marked, its editors attach read-only (`Open::marked`, refreshed as
  each join answers), so nothing is typed that would be lost unseen. The
  replicas tell the worker as a replica opens and once the replica a join
  opens has its editors attached as the join says (`Host::set_marked`), and
  the discard tells it too, and the stack seeds it from the refs database's
  marks as it attaches its peer, so a restart keeps it before any replica
  opens; the page reads it as `Seen::marked`, which caps
  `be_host_stack::access` at `View` and so caps graph commands
  (`Instances::editable`) as well as editors, so app editors are told they are
  read-only and then editable again (a test of the stack waits for `access`
  rather than for a snapshot, which a reopened replica does not send again); keeping
  its edits fails (`ClientError::Discarded`), so a submission waits rather
  than going out unkept; a seal answers unchanged
  and its commit never reaches the outbox (`Refs::push_publish`, checked as
  the discard checks), so what the closing replica held is sealed into
  none; and a commit sealed before the discard that the server refuses after
  it is dropped rather than set aside again (`Refs::move_to_unsent`). Nothing
  typed after the account lost access reaches the server once it may edit
  again. Opened before it
  joins (offline, or joining), those the server may hold already only show
  (`opening.doubtful-through`, the replica's `doubtful` layer): numbered here
  they could land twice. Once joined, the order says which it holds, even after
  a seal trimmed the entries that held them: the journal's base and each reload
  carry each recent origin's highest `through` (`wire::BaseState`, sealed with
  the rest of the base). Kept edits made before a restart the order had since
  (`opening.kept-restart`, against the base's last restart) are dropped, each
  told to its editor as not sent: a restart, the last one out leaving included,
  makes every edit kept from before it fail visibly.
- **A lost entry.** The host tracks the last number of the order it heard per
  open block (`Open::heard_through`); an entry past the next makes the replica
  rejoin and open again on the server's journal.
- **Opening early.** A block this device knows opens from its local head in a
  session a placeholder holds (`JOINING_OWNER`) if joining takes longer
  than `EARLY_OPEN`; when the join lands (`Io::Joined`) the replica is reopened
  with the real session, replaying what happened meanwhile; an editor that made
  an edit kept for that and has detached since is attached for its edits to be
  given again (`Open::departed`), or the replica would drop them. A seal asked of a
  joining replica with unconfirmed edits first gives it a made-up session whose
  lease it holds, so flush and stop never wait for a join.
- **Opening offline.** A block opened while down gets a client of its own,
  made for that open with the top bit set (`unjoined_client`: never the sequence
  library's `LOADED`, whose edits `Sequence::replace` refuses, and never another
  device's), holding the lease of a made-up session (`Open::made_up`): `Submit` is numbered in this
  device's own order (`Peer::submit` offline) and heard by the replica at once,
  `Publish` becomes `Peer::seal` (a local commit in the outbox), `Resume` treats
  the local head as remote, and claims, trims of the server's journal and relays are skipped. It opens on the newest head whose content the device holds
  (`Peer::held_head`, guides/blocks/client.md), not the server's newest, which
  it may never have read. The same happens while the block
  has outbox commits, and while the host keeps (`Replicas::keep_all`, used by
  flush and stop). Online, `Resume` reconciles with `Peer::newest_head`: the
  local head while the block has outbox commits (a definition the worker wrote
  beside an open replica), so the replica seals on top of them, not beside them
  where the server would refuse one side.
- **Draining on connect.** The worker drains the outbox in a task
  (`Tasks::sync`), merges each `Diverged` block (`Replicas::merge_diverged`:
  `Peer::reconcile`, then the module's three-way merge, published with the
  local commit as an extra parent), and only then rejoins open blocks. It
  drains again after restoring unsent edits, after keeping, and every
  `OUTBOX_RETRY` while something waits.
- **Session calls off the loop.** Relays, claims, leaves, unwatches
  and resynchronizing joins are answered in tasks (`Io::*`). They are written
  the moment they are made, so a leave and the next join reach the server in
  order; an answer older than the replica's session is dropped.
- **Locked messages.** A relayed message is unsealed on the spot; one whose key
  is missing (`ClientError::Locked`) waits, with that block's later events
  behind it, for one search per key (`Replicas::searching`), started by the
  first such message whatever else searches for the key (`Peer::learn` may
  answer only after it arrived). Once a search for its key has answered
  (`searched`, cleared when a search opens keys and on reconnecting), a message
  still locked is dropped and recorded, so a peer sealing with a key this device
  cannot get costs one search.
- **A module that restarts** has its replicas opened again on this device's
  journal and, in a live session, joined again before it is given their edits
  (`Replicas::restart`), so the replica opens on the server's journal: this
  device's copy was never trimmed by another device's seals, and an edit on its
  way when the module stopped is found in the order rather than made again.
- **Stopping**: `be_host_stack::stop`, `stop_and_wait`. Tests stop as a crash
  would with `crash`: the worker ends without keeping the open blocks, so only
  what the device had written survives.

`be_host_stack::status()` reports `connected`, `syncing`, `waiting` (outbox
operations), `unsent`, `conflicts`/`conflicted`, `kept`, `connecting` and
`unsaved` (edits this device does not keep yet, `Replicas::counts`' third
number: neither a journal holds them nor its kept edits, which are written
ahead of each submission; plus every command sent and not yet carried out,
`Shared::pending`, and imports; a kept edit survives a restart, so one the
server does not hold yet, or no commit, is not counted, and nothing settled
waits on a server that does not answer). The "Saving changes" toast follows `unsaved`.

The close and switch guards ask only while `be_host_stack::flush()` returns
more than zero. It returns the worker's count as it answers the flush: open
blocks whose edits the journal does not hold, commands sent before the flush
that still wait in `Uploads` or `Served`, and unfinished imports. Commands sent
after the flush do not count, so a window that keeps sending as it is asked to
close still closes. On a timeout, and on the web, which cannot wait, it returns
`unsaved` as read before the flush.

Not built yet: a closing web tab cannot wait for `flush()`, so it asks first
(`beforeunload`) while anything is unsaved (`Replicas::counts`).

**One process per data directory.** The refs database keeps one origin per block for this
device (`refs::OpenEdits`), so two processes on one data directory would number edits under
the same origin. `be_host_stack::refs_in` locks `be-refs.lock` in the directory for the
process: the hosts of one process share the database, and another process is refused with
"Another be-host is using this data directory". The be-host binary hands a second launch over
to the first before that (`instance.lock`). The web build takes no file lock: the `block-files`
Web Lock already keeps one tab the owner.

**Create, define, move and duplicate** are kept on the device and wait in the
outbox, online as offline (`Peer::keep_creation`, `keep_definition`,
`keep_move`), and the loop starts a drain for them; the loop never waits on the
server for them. What follows such a block waits behind it: its publishes stay
in the outbox while a creation, move or define of it waits
(`Peer::publish_sealed`), a block made under it waits behind its parent's
creation (`Peer::create_block`), `Peer::summary` answers from the device until
the server knows it, and a listing the server answers is laid over with what the
outbox adds or moves (`Peer::listed`). A block opened before its creation is
sent opens as offline does; once a drain sends it, the replica seals what it
ordered and joins (`Replicas::created` asks for that seal once, while it owes one
and none is on its way). A command that needs the server to know a block waits
first for a drain to send what this device made of it (`Tasks::sent`; an
export, and a version command in the lane, waits for the whole outbox, since it
reads a tree).

**Commands that need the server.** Those that need only the server and no open
block (share, list access, read a head, leave, approve pairing, members, block
agents; `Command::served`) run in a lane beside the loop
(`worker/served.rs`): one at a time, in the order sent, each in a task, so a
server that stopped answering holds up only the lane. They count as unsynced,
not busy (`Shared::serving`). Offline, sharing, leaving and approving a device
are refused to the caller, who shows it where the person acted; they are not
kept for later and are not the host's error. Order: a loop command waits while
the lane holds a block it needs (`Command::needs`); one that is not order-safe
(`Command::order_safe`: it may reach blocks it does not name, as a version
command or an import reaches a tree) waits until the lane is empty; and none
overtakes a command deferred before it. An open block whose key the lane
rotates is resealed once that command ends. A version command (merging
included) is split: the loop seals the open replicas, and the rest runs in the
lane (`Served::version`), which waits for the outbox to be sent and then reads
and writes the server between steps, reaching the replicas and type modules
through the loop (`version::Reach`, `LoopCalls`: describing, applying and
merging content, moving an open replica to a head it published), so a server
that stopped answering holds up only the lane. A merge reads its sides from the
server in the lane first, so the loop finds them on the device.

## The graph mirror

`be-host-stack/src/graph.rs` mirrors the part of the server's graph something
in the host is looking at: a `Node` per block with type, author, parent,
access, references, unsealed `BlockMetadata`, `locked` (no key opens it), the
server's `size` and `tree_size`, and its `changed` and `entry`. The host never
lists the whole organization.

No summary names what a block references, so the tree's shape comes from
parents and a node's `references` (in its content's order: a folder's order,
its links) from its content: its open replica's report (`Graph::refer`), or
else read from its head by the worker (`read_references` in worker.rs,
`Peer::references_of`) and kept per head (`Graph::read`, which a replica's last
report joins as it closes), or for a block with no content yet its metadata's.
They are `None` until read. The worker reads them for every node it holds that
this account may read (`Graph::take_to_read`: what is asked for first, at most
`READS_TAKEN` a take, `READS_AT_ONCE` at a time, skipping a head that moved on
while it waited, and only after something changed, which costs one scan of the
nodes), so a folder's order, and whether a collapsed row in a
file tree or picker has anything to expand (`BlockInfo::references`), are read
without opening it; `ready(Query::References | Query::Children)` waits for the
first read. Until then a child is held as the server says (`block-summary.held`,
`Graph::holds_in`), and backrefs are the server's listing. At a new head the
references read at the last one show until it is read, and a child the server
says is held counts as held meanwhile, so one another device just added never
reads as deleted. Offline the read answers from content this device kept (the
vault holds the commits it read); a read that failed for a reason that may pass
(a fetch cut off) is tried again on the server's next answer, `READ_ATTEMPTS`
times; then, or at once for one that lasts (no key, forged, undecodable), the
head is not tried again until a new connection or new keys
(`Graph::read_failed`, `unread`, `keys_changed`), and its `References` query answers the server's listing
meanwhile, in the order of ids.

The stack tells the host what changed rather than being polled: it pushes a
`Changes` notice (the graph, contacts, storage, stale blocks, search answers,
notification modes, and the blocks whose content changed) into every
`ChangeWatch` (`be-host-stack/src/changes.rs`) as it happens, merged until the
watcher takes it, and the first change after a take wakes the host; a new
watch, and every watch when the stack starts or stops, hears everything.
be-host's core, each app runtime's instances, the compositor and the input
remapper each keep one, take it once a frame, and read again only what it names.

The account's storage against its quota is not in the graph: while an editor
watches `StorageUsageWatch`, be-host turns on `be_host_stack::watch_storage`,
which follows the server's `account.storage-changed` stream (opened again on
reconnecting) and publishes each value; a stream that ends while connected
sets the stack's error.

Everything asks the mirror through `Query` (`Roots`, `Detached`, `Shared`,
`Children`, `References`, `Backrefs`, `Parents`, `Block`, `Subtree`), and
asks again when a change notice says the graph changed. Askers declare what
they want with `be_host_stack::want(owner, queries)` (`Owner::Editor` per app
instance, `Owner::Open` for open blocks' parents, `Owner::Host`,
`Owner::Version`).

- **Listings.** The graph turns what is wanted into its `Listing`s (the
  server's, and `Block` for one block's summary), expands `Parents` and
  `Subtree` as blocks arrive, and fetches new ones together in a task
  (`spawn_listings`), so a slow listing never holds up an open or an edit. The
  loop's other background requests are `worker::Tasks` too.
- **Dropping.** A listing nobody wants is dropped with every block only it
  held; the server keeps nothing for it.
- **Readiness.** A query answers only once its listings loaded
  (`be_host_stack::ready`), so an editor never sees a loading folder as empty.
- **Following trees.** Each node names its entry point (`Node::entry`, the
  server's `block-summary.entry`), and the graph counts the nodes it holds in
  each (`Follow::nodes`, kept as nodes are inserted and removed, so following
  costs nothing per change beyond the node changed): the first node in an
  entry's tree follows it from the sequence it was read at, the last one out
  unfollows it (`take_follows`). So the host follows only the trees of what it
  shows, a few entries, never a listing. `Event::TreeAdvanced` marks the entry
  to read (`Graph::advanced`, once more if it advanced during a read), and
  `read_trees` reads its `tree` pages from its cursor, in a task: the
  graph takes only what it holds or a listing it holds would list
  (`Graph::followed`, `holds_or_lists`), so a page's changes in folders nobody
  shows cost nothing more; a block a page changes has its `Backrefs` listing
  read again (the server touches the blocks a change starts or stops
  referencing); a held block that left the tree is read again wherever it went
  (a `blocks` query) or forgotten. A read refused (the tree is no longer the
  account's) reads again the listings holding what lies in it
  (`follow_failed`), so what was shown of it goes. Every head this device
  learns moved on from one it knew, by any read, reaches the open block's
  replica with the next pass of the loop (`Peer::take_moved_heads`,
  `Replicas::heard_head`). `Event::Roots` lists again the top, the trash,
  Shared with me and the open settings, tries again every tree whose read was
  refused, and reads again the blocks the account may only know exist, which
  name no entry and so are in no tree it follows (`Graph::roots`,
  `known_only`). Reconnecting or falling behind re-lists everything and
  reads every followed tree again from its cursor. The mirror keeps the newest
  `changed` per block (`heard`) and drops anything older, so a late answer
  never undoes a newer one.
- **Local changes** (`create`, `set_parent`, `define`) apply to the mirror at
  once and are pending for that block: the mirror ignores the server's echoes
  until the change is settled, then refreshes. An open block's references come
  from its session's content, so a new child is listed before it is sealed.

## Metadata

`be_block_content::BlockMetadata` is two maps from key UUID to CBOR: `defined` (chosen
by someone: `NAME`, `LOCAL_ID`, the artifact source) and `derived` (said by the
content: `INTRINSIC_NAME`, `THUMBHASH`, `SCHEMA`). Keys are typed constants
(`MetaKey::defined`/`derived`), so a new key, a third party's included, needs
no format change.

A value holding a `BlockRef` is a reference the peer publishes with the block,
so backrefs and `KnowExists` see it and a parent holds what its metadata points
to (its settings, an edit kind). Such blocks are not children: apps get only
what the content refers to (`BlockInfo::references`), so a folder never lists
its settings or tags.

Metadata travels with content: every commit carries it, and `Publish` carries
it sealed with the block's data key, so the server stores it in the step that
moves the head. A save takes `defined` from the head it builds on and
recomputes `derived` (`be_block_content::derived_metadata`); a merge merges `defined`
key by key (`BlockMetadata::merge_defined`). Unknown defined keys ride along;
unknown derived keys are dropped on a new save, since they describe old
content. `Peer::define` publishes a commit of the current content with new
metadata, or a `publish` with no commit on a block with no content yet;
`be_host_stack::define` and `set_name` apply it to the mirror first, and
`describe_implicitly` keeps an open block's derived metadata current until it
is saved.

## Unread counts

A type that counts what the account has not read (`be_block_content::Unread`,
guides/type_modules.md, "The protocol"; chat's new messages) gets its count on
its node, `Node::unread`, and so on every `BlockInfo` an app is told
(`unread`), without a replica opened per row. `be-host-stack/src/unread.rs`:

- **Reads.** Where the account read up to lives in an owner-only settings kind
  the type names (`type-info.reads`, `KnownKind::reads`). be-host resolves the
  account's layers of each such kind (`unread::kinds`,
  `RootSettings::layer_contents`), holds them, and sends their content's bytes
  to the worker when they change (`unread::publish`, `Command::Reads`, and
  `Start::reads` for a stack that starts), as it does the offline choices.
- **Counting.** Each pass (`unread::step`) looks at the nodes once something
  moved (the graph's revision, the reads, the known kinds): a node this
  account reads whose type counts and whose count at its head with the reads
  now is not known has its head loaded in a task, as the type loads it (a chat:
  its header only), and its module asked (`Replicas::unread`). A node read
  again from a listing keeps its count (`Graph::insert`) until it is counted
  again. A type whose manifest says `counts_unread` has its module started
  once a node of it is held. A head that fails is not tried again until it or
  the reads move.
- An open block is counted from its head too, so a message counts once its
  session seals it (at a pause), not as it arrives.

Tests: be-host-stack's
`a_chats_unread_count_is_counted_from_its_head_and_the_accounts_reads`;
be-sdk-test's store counts the same way (from the head, with every block of
the reads kind it holds), which be-app-file-tree's
`a_chat_row_counts_its_unread_messages_until_they_are_read` uses.

## Search

Full-text search over names and content (`be-host-stack/src/search.rs`; what is
set aside: plans/deferred/search.md). The server never reads content, so clients build the
index and search it locally.

- **One index per root block** (a workspace, My files): a block of
  `be_block_graph::SEARCH_INDEX_TYPE` under the root, named by its
  `SEARCH_INDEX` metadata, so it is held by the root and never listed among its
  children. Its access is exactly the root's, whatever its own authorship says
  (`BlockNode::is_search_index`, `level_below`), so a block shared alone never
  reads its tree's text through it. The server holds to the type and parent,
  since it cannot read metadata: an index lies under a top-level block, one held
  per block, never moves under another block (it would read that tree's text as
  the new parent's), takes no grants of its own, and making or dropping it needs
  Edit on the root. It rotates with the root's subtree like any block below it.
- **A block that leaves the tree** (moved to another root, trashed, its text
  deleted) is not masked yet: its text stays in the old root's index, which
  every viewer of that root can read, and only `show` hides it by access. Treat
  a moved or trashed block's old text as still readable by that root's viewers
  (plans/deferred/search.md, "Limits accepted").
- **The content** is `be_block_search_index::SearchIndexContent`, a
  `be_lib_search::Index` kept as the block's parts: the header first (all a
  replica loads, `Loads::FirstPart`), then every part the header names, read by
  range. Two sides merge by parts (`MergeParts`, the union `Index::merge`
  makes); it is registered with the host's own types (`be-sdk-type-host`).
- **Indexing.** A head this device's session moves (`Host::status` with
  `owner`) queues the block (`Searching::sealed`). Each round of the worker's
  loop takes the queued blocks whose root it may edit, reads each one's name
  from the mirror and its documents' text at its head with `search-text`
  (`Replicas::search_text`; a type that answers nothing is found by its name),
  and hands each index one batch in a task, which applies it to the index's
  newest head and seals the commit into the outbox: it is saved offline-first,
  published by the next drain, and two devices' batches meet as a diverged
  block that merges by parts. A workspace (`WORKSPACE_TYPE` at the top) with no
  index gets one as soon as a device that may edit it sees it in the mirror,
  and the mirror has no change made here to it still waiting for the peer
  (`Graph::changing`): My files is in the mirror before the peer keeps its
  creation, which pins the root of its log that the index's parent record
  names, so an index made in between is refused as locked. Settling the
  change moves the graph on, which looks again. The index is
  made in a task, then named in the root's metadata from the loop once the
  root's open replica has sealed its edits, as a define command is, so the
  define never lands under a moving session. The loop takes one block a round
  and asks only a type whose module already runs: starting one, or retrying a
  define each round, would stall the opens and edits behind it. A document's
  text is indexed up to `MAX_TEXT` (1 MiB) bytes. Blocks whose index has no
  head on this device yet are parked until it has one, not queued each round.
  The host is busy (`Searching::busy`, which tests settle on) while a workspace
  it may edit wants an index and has none, while one is being made or named, and
  after a define is sent until the root's summary is read again, so the size the
  files pane shows is the server's with the define in it; a define still in the
  outbox after a drain that ended since is not waited for. Naming is tried at most
  eight times, and a refused define gives up on the root with an error shown, until
  this account's access to it changes; a root whose index could not be made is tried
  again when the mirror next changes or the device connects again, with no error
  shown when the server could not be reached or did not hold the root yet
  (`search::not_there_yet`). A root that was left or can no longer be edited
  drops its index and the blocks waiting for it.
  `Searched::complete` is true when nothing this device could index is queued,
  being made or written; an edit on a root it cannot edit is never queued.
- **Compaction.** A batch's write also compacts a few segments
  (`SearchIndexContent::compact`), none whose merge reads more than
  `MERGE_BYTES` (8 MiB), so no edit waits on a large merge. What the cap leaves
  (`SearchIndexContent::merges_over`) is merged while the host is idle
  (`compact_idle_indexes`): nothing queued, written or caught up for search, no
  search running, no command waiting, the outbox empty and no module working.
  One index at a time, a commit a pass, with merges up to `IDLE_MERGE_BYTES`
  (128 MiB, since a merge reads its segments whole into memory), sealed and sent
  like a batch (it counts as one being written, so the host is busy meanwhile and
  that index's batches wait). Only the device that wrote an index's last batch
  compacts it: `Searching::last_written` keeps the head each write here left,
  and the pass looks only at an index still at that head locally and on the
  server as far as this device knows, once per head, so two devices never make
  the same merges and nothing is loaded for an index no write here moved. There
  is no signal yet for the device being idle or on power, so this is the host's
  own idleness.
- **Catching up.** When this device is connected and may edit a root whose
  index is named, `start_catch_ups` asks `Peer::changed_since(root, watermark)`
  (the index's `Index::watermark`) once per connection: the blocks it lists
  (commits from devices that never index, imports, blocks moved in) are queued
  like a sealed block, read from what the call said where the mirror lacks them
  (`Searching::caught`, a type's module started for them), the blocks that left
  are masked (`Batch::removed_blocks`, at the time of the catch-up; a block that
  comes back is stamped after the mask), and the watermark moves with that batch
  once everything the catch-up queued is indexed. A block a catch-up listed whose
  commit cannot be read holds the watermark back; after `MAX_READ_FAILURES` reads in
  a row it is let go, so the watermark moves past it, and kept in the refs database
  (`Peer::let_go`, this device's own state, never in the index) to be read again when
  each later catch-up starts, after a restart too, until it is read or is no longer
  indexed. A block sealed here whose read fails is not retried at once: it waits
  for its next change or a catch-up that lists it. A block listed before its
  content was published is caught by the next catch-up, which is after the next
  connection. The host is busy while one is due or running. A search that is
  shown is shown again whenever the mirror moves, so a hit whose block left what
  the search is within drops out at once.
- **Searching.** `be_host_stack::want_searches(owner, asks)` names the searches
  an owner shows and `searched` answers each (a change notice says when one
  changed). A search runs in a task over the index of every root the account
  views in the mirror (or of `within`'s root), reading only the parts it needs
  with `Peer::read_part`, and again whenever one of those indexes' heads moves.
  The loop then cuts the shown hits' snippets (`Query::snippet`) from their
  documents' text (`search-text` with the hit's key), kept per document and
  head, and drops hits the mirror shows
  detached, unviewable or outside `within`; it lists each hit block
  (`Owner::Search`) and shows a hit only once the mirror holds it.
- **Opening a hit.** Quick open (`OpenableBlock::at`) and the file tree's hit
  rows (`EditorHost::open_block_at`) open a hit's block with a
  `search-location`: its document key and the words its first match covers. A
  phrase matches only where its words are in a row, as one range
  (`Query::matches`), so the first match is where the text first matches the
  query and the first place its words occur is the hit's.
  be-host keeps it (`reveals::Reveals`) until the editor showing that block
  is open, a few seconds at most, then sends it `editor-event.reveal`; a later
  open of the block, or its editor closing, lets go of it. be-sdk hands it on
  as `Pushed::Reveal` and `EditorHost::take_reveal`. The text app, once its
  content has loaded and its saved position is back, selects the first place
  the words occur as whole words, compared as the index folds them
  (`be_lib_search::fold`); an editor that supports no location ignores it.
- **Apps** watch a search with `search-watch` (`Editor::search` in
  be-sdk-beui); the file tree's filter lists the hits under "In content", and
  be-host's quick open (`found_blocks` in `compositor/palette.rs`) lists them
  after the blocks whose names match.

## Creating, seeding and replacing

- **Creating.** `create-block` carries the first content, written as the first
  commit before any later command for the block; with none the worker writes
  the type's blank content (`Kind::blank`), so a host or app can make a block of
  a type whose format it does not own. Apps pick the new id themselves.
  `be_sdk_database::create_database` makes a schema and a database together.
- **Seeding.** `editor.seed_content` (`write-content` without `replace`) writes
  a first commit, and is ignored for a block that has content or is open.
- **Replacing.** `editor.replace_content` (`write-content` with `replace`)
  becomes the replica's `Change::Replace`, or a direct publish when nothing has
  the block open. The replica stashes it as a commit (the bytes take the chunked
  upload, not the session) and submits a barrier reload of it, and the
  replacement wins: unsaved work is stashed as a parent of a later head, and
  every replica reloads it (guides/type_modules.md, Peers). Image, audio and PDF
  "replace" and compiled logic use it.

## Exporting and importing

A block file (`be_block_client::transfer`, what `be download` writes) holds a
tree. host-services' `export-block` lists the tree a level at a time and fills
each block's bytes from its open session, or its head with the journal
replayed (`Modules::readable`), so it works for types the host does not know
(they read the head alone).

`import-blocks(parent)` takes any file: non-`.block` files go to
`be_host_stack::import_file`, which picks a type the way `be sync` does
(guides/sync.md); a `.block` file is `be_host_stack::import`ed with fresh ids
swapped into content and references. The tree is in the mirror at once and
uploads in a task beside the loop (`worker/uploads.rs`): `Peer::queue_import`
mints keys and writes commits into the outbox (so an offline or interrupted
import is sent by the next drain), then `upload_import` sends everything
without waiting on each answer. Commands on a block still uploading wait for
it, and anything that rekeys a subtree waits for all of it. Import straight
into the target folder: moving a tree into another workspace rekeys every
block in it.

## Children, duplication and holding

Moving a block into a container, deleting a child, replacing it or moving it
among its siblings asks the container's content to change: `be_host_stack::change_child` sends the
container's replica `Change::Child`, and the type's `Root::child_edit` (or
`child_edit_in`, given the document and the replica's client) turns a
`be_block_content::ChildChange` into an ordinary edit, or answers that it takes
none. `change_child` hands back a `ChildChanged` that says so once the replica
answers (`Replicas::change_answered`, awaited beside the worker's loop), and
be-host's moves, reorders and copies each wait on it on their frames: a
refusal puts the block back where it was and says why (`refuse_transfer`,
`refuse_reorder`, `refuse_copy` in its main.rs). Unlink
(`BlockCommand::Unlink`, `HostCore::queue_copy`) asks the container's type to
put a new copy in the block's place (`be_host_stack::replace_with_copy`), only
for a type whose manifest says `children.replace`. The stack's worker makes the
copy (`duplicate(from, to)`) as soon as the type takes it, before it answers, and
gives the container its source back if the copy cannot be made, so the
container never names a copy that does not exist; a refusal leaves none behind.
The host then places the copy in the container and shows it; an editor of the
container never takes part. An editor asks for a reorder with
`EditorHost::reorder_block(block, parent, after)` (`BlockCommand::Reorder`);
be-host queues it behind any move of the same block still under way
(`queue_reorder` in its main.rs) and sends it only to a type whose manifest
says `children.reorder`. `be_host_stack::duplicate`
copies what the source's replica shows (or its head) into a new id before
anything opens the copy. `be_host_stack::hold` keeps a block open for the host
(settings, the shell's session).

A block being placed counts as held by its parent (`Graph::placing`) until the parent's
references take it in. Should they drop it again while it still names the parent, it reads as
deleted: the mirror notes it (`Graph::take_dropped`) and the worker logs it with what it knows
of the parent (`Shared::log_dropped`), which a host test's failure shows in the app's log.

## Undo

Undo lives in the block's replica, the one place that sees every edit from this
device, whichever editor made it. The host's own edits (`be_host_stack::operate`, which it
makes as `HOST_EDITOR`, setting up a new organization's settings say) are no one's to
undo: the replica keeps no step of them. A type registered with `undo` keeps
`be_block_content::history::History` (`HISTORY_STEPS` deep): steps sent within 750 ms
of the last, by the frame-clock time each relayed frame is stamped with, may
absorb into one, and `be_host_stack::undo`/`redo` send `Change::Undo`/`Redo`.
An editor groups a gesture into one step with `FromEditor::Gesture { id, gesture }`
(`ContentProjection::begin_gesture`, then `operate_in`, `commit_gesture`,
`cancel_gesture`; a cancel reverts and leaves no step). Each gesture is named by
an id the editor picks (be-sdk numbers them per `EditorHost`), and each edit
names the gesture it was made in (`FromEditor::Operate { gesture, .. }`), so
one editor holds several gestures at once (two fingers, two controllers), each
ending on its own; an edit naming none is recorded outside every gesture. An
editor ends its burst with `break_step` (`FromEditor::Break`), so its next edit
starts a step however soon it comes. A text
editor breaks where its core ends a group: at word boundaries (so quick typing
undoes a word at a time), a paste, a caret move, formatting, blur; never for a
peer's edit. A break leaves the editor's open gestures alone, and another
editor's burst. `History` keeps each gesture under `GestureId` (its editor and the
protocol's id), so gestures stay open side by side and each commits or cancels on
its own. It also keeps tentative gestures (`GestureKind::Tentative`, opened by
`Gesture::BeginTentative`, `ContentProjection::begin_tentative`;
guides/type_modules.md, Tentative gestures), whose edits are held until the last
open one ends and then become one step. When the replica ends a gesture itself
(an undo cancels the layer, a replacement or a restart drops what it moved) it
tells its editor `ToEditor::GestureEnded`; `EditorMirror` answers with a cancel
of that gesture, which is what ends it for the host's `Holds` and the replica
alike, drops what the editor makes in it after, and the projection runs
`on_gesture_ended` so the editor stops the gesture. Only a commit already on its
way when the replica ended it crosses that word. An edit the replica drops in an
ended gesture is answered with a snapshot, so the editor's mirror lets it go. An
undo or redo commits open undo groups first; while a
tentative gesture is open, an undo cancels the whole tentative layer instead and a redo
is refused. The runtime keeps the ids of the gestures each link holds open and cancels every one when the link goes. A session restart keeps open gestures open with what they recorded. Apps call `act(block, block-command::undo)`
and watch a block's `history` with a `block-watch`. A session restart (guides/type_modules.md, Peers)
drops every step, since none applies to the reloaded content: `block-history`
says `restarted-at`, and the frame answers an undo with nothing left by saying
"Can't undo past here" with a button that sends `block-command::history` to
show version history at that commit (the shell gets `show-request::history`).
`lost-edits` says edits of the block made with another build were lost (its
journal held some this build cannot replay, or the server marked them lost since
the last commit); the frame says so.

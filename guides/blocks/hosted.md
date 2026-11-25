# Hosted blocks

Part of the block stack (guides/the_new_block_stack.md). A hosted block is run
by a host: one member with Edit, online, holding the block's live session, that
others talk to instead of reading the block. A game world is the shape: the
block holds the whole world, secrets included; players see only what the host
sends them and act only by asking it. Chat run by a bot, a private poll or an
exam that hides its answers are the same shape. The host is any member with
Edit (decisions/a_hosted_block_is_hosted_by_any_editor.md): share a world with
one editor and two players, and whichever editor is online hosts while the
players join. `crates/be-app-gem-hunt` is the example ("The gem hunt").

Where each part is built: the level and its grants in be-block-graph and
be-server (guides/blocks/server.md, "Access"), routing in the session
(guides/blocks/server.md, "Live sessions"), the channel's handshake and seal
in be-block-keys and be-block-client (guides/blocks/keys.md, "Hosted
channels"), and the host component and the Join editor on the app protocol
(guides/apps/host_services.md, "Host components"; headless in `be`:
guides/apps/runtime.md, "Headless").

## Access

`Access::Join` lies between `KnowExists` and `View`:

| Level | The stored block | The host |
|---|---|---|
| KnowExists | id and type (the name is sealed metadata) | nothing |
| **Join** | nothing: no keys, no objects | a private channel to it |
| View | all of it | a private channel too, and the whole live session |
| Edit | change it directly | may be it |
| Manage | grants and settings | may be it |

Join is granted like any level and, like `KnowExists`, reaches only its own
block, never the blocks below it. A Join member is served no keys or objects,
and lists the block under Shared with me by its type, since its name is sealed
metadata it holds no key to (`Query::Shared` lists Join and above,
`BlockInfo::reach` counts Join as reached); opening it opens the type's editor
as a Join member (`opening-kind.join`), whose view shows what the host tells it.

Edit means "may change the stored block and so may host it". What someone may
do inside the world (move a piece, kick a player) is the app's own role, kept
in the block's content and checked by the host; a powerful player is a Join
member with a role, not an editor.

## The share dialog

A hosted block type says so in its manifest: its host component's `levels`
(`HostManifest::levels`), which levels the share dialog offers, each with a
name and one line saying what it means; every level choice and name in the
dialog goes through be-sdk-shell's `Offered`, and people added, contact links
and share links start at Join. A type that is not hosted never offers Join; a
folder's dialog shows the core names. On a hosted block the dialog says under
"People with access" who hosts it now, or that no one does
(`access-listing.host`), as of when it last listed.

A type names a level by what it adds for the type's stored content, never by
a role inside the app, since the share dialog grants no roles. A game:

- **Join** ("Join game"): connect to whoever hosts. Playing, watching,
  building or moderating are roles the host gives, possibly through a setting
  of the world; someone who should build in a shared world needs Join and a
  role, not Host.
- **Host** (Edit): "Can run the world, and so can read and change anything in
  it, outside the game's rules. Hosts only while the world is open, or after
  Keep hosting." The line must say both: that the level reaches all of the
  stored world, secrets included, and that it is a permission, not a duty to
  keep a machine running.
- **Manage**.
- **View**, only if the type offers it, named for what it is ("Read the world
  data": every secret and the whole history, for a co-designer, a backup or a
  review), never as watching the game, which is a Join role.

## The channel

guides/blocks/keys.md, "Hosted channels", has the cryptography; what it
guarantees:

- **The host is an account.** The server says which account holds the lease,
  and the joiner checks it on the signed logs, at Edit or above, taking the
  X25519 identity key the record giving it names, so only that account's
  devices complete the handshake and the server can neither stand in a host of
  its own nor pass the lease to one without Edit. No key derives from the
  block's data key: anyone who could read the block could compute it, a View
  member included.
- **Handshake.** Noise IK from the joiner's account key to the host's, with the
  block and the log head the host was checked at as the prologue, so both agree
  on the block and the state of its access. The host admits a member whose
  grant checks (Join on the block, or View or above anywhere up its path) and
  who is not banned, once its first channel message opens. Later messages are
  sealed in order, so the server can neither read, forge, reorder nor replay
  one; a host change, a reconnect or a message out of order starts a new
  handshake.
- **The server routes.** A Join member joins the block's session as a member
  of its own kind (`member-kind.join`): it hears only what the host addresses
  to it and whether a host is online and which account, its messages go only
  to the lease holder, and only the host sees it come and go. View and Edit
  members use the same `host` send, so no member reads another's messages to
  the host; what each sends the holder is rate limited
  (`limits::HOST_MESSAGES_PER_SECOND`). A member admitted by a link may message
  the host though the two are not contacts: the link was the out-of-band start
  (decisions/share_links_are_an_out_of_band_start.md).
- The same handshake works over a direct connection a game sets up later; the
  relay is only the first transport.

## Hosting

- **The rules are in the app.** The type module stays as simple as text's:
  storage and basic operations on the world, nothing game-specific, since every
  editor runs it as the replica. The rules (what each player sees, which moves
  are allowed, timers) live in the app's host component: an entry point of the
  same app wasm with no frame, attached to the replica as an editor with Edit
  is, that hears members join (account, level), speak and leave, and tells one
  member at a time (guides/apps/host_services.md, "Host components"). The Join
  side is an ordinary editor with no content, only the channel and whether a
  host is online. Neither side sees keys, and the protocol names no framework
  types.
- **A host writes as it goes.** When the host leaves, another editor takes the
  lease and the joiners reconnect to it; the world continues from what is
  stored, so only what the host held in memory is lost.
- **When a device hosts.** be-host runs a block's host component where an
  editor of it is open, the account may edit, and no other device hosts it
  (`crates/be-host/src/hosting.rs`); closing the last editor of the block
  stops it and gives the lease up, so a world left closed uses nothing.
  Hosting on after the editor closes is explicit: Keep hosting in the frame
  menu (`editor-event.hosting`, `block-command.keep-hosting`), after which the
  host menu lists the block among what the device hosts, with Stop. The list
  is kept in memory only, not in settings or view state: quitting be-host stops
  every host, and starting again restarts none. On a server, `be BLOCK host`
  runs the same component headless while no other device hosts the block.
- **An editor that is not hosting** may play through the host like anyone
  else or change the block directly; the app decides which its editor offers.
- **Banning**: a signed ban record in the access log (account, signer with
  Edit), which the host refuses at the handshake and the server at joining. Any
  Edit holder may ban: an editor can write anything to the block and host with
  its own logic, so a ban never binds editors, only who joins through an honest
  host or server. A ban and a grant to the same account are ordered by their
  place in the log: a grant signed after a ban (by Manage, as every grant is)
  lifts it, and a ban after a grant bars the account while the grant stands.
- **Who is admitted stays checked.** be-host-stack drops a member's channel
  when it leaves the session, when a message of its does not open (taken as its
  new handshake), when the component falls `EVENTS_KEPT` behind, and when the
  signed logs no longer admit it, checked again on each change of keys, roots
  or the tree, a resynchronized connection, and this device's own change of
  access or ban (`crates/be-host-stack/src/hosting.rs`). An editor whose access
  crosses between Join and View is closed at the app and opened again as the
  other kind, so a participant that falls to Join handshakes again as a member.
  The app may show which account hosts, since it is checked as one
  (decisions/share_links_are_an_out_of_band_start.md).

### Who hosts

The session's lease (`be-block-session`'s holder: the editor that claimed
last, else the first editor) decides who seals the journal; the host is the
claimed holder only, or no one when no editor in the session claimed
(`Membership::claimed_holder`, `SessionRegistry::host_route`). So every
session keeps a sealer whenever an editor is in it, while members wait rather
than handshake with a device that holds the replica open for another reason (a
child, a preview) and runs no component. A device claims as its component
starts; of two that start at once the later claim wins, and the other, hearing
another host, stops its component. A component that stops while the device
stays in the session has its replica join again, which drops the claim; one
that stops with the last editor drops it by leaving. A claim the server refuses
leaves the component running unhosted, with no Keep hosting offered, until the
block stops being one the device should host.

## Accepted limits

- A Join member checks the host on a proof path, which can hide a later
  revocation of the host's Edit behind a hash (guides/blocks/keys.md, "Accepted
  limits"). A server that hides it can keep routing to a revoked editor, who
  then reads the players' moves; it reads nothing stored, having lost its keys
  at the rotation.
- A Join member holds no key of the block, so nothing carries it the log's
  root: it takes the root as served once a contact it verified signed a grant
  to it there, or by the grant it signed itself through a share link, and pins
  it weakly, in memory only.
- A leaked recovery phrase lets its holder host or join as the account.
- The share dialog names who hosts as of its last listing: it lists again on
  each change it makes, not when the lease moves meanwhile.

## The gem hunt

`crates/be-app-gem-hunt` (types in `crates/be-type-gem-hunt`) is the first
hosted type: a six by six world with six gems hidden in it. The block holds
where the gems lie (`Hunt::gems`) and who dug each cell (`Hunt::dug`); the type
knows nothing of the rules but `may_dig` and the counts. Its pieces:

- `HuntHost`, the host component, keeps the members it heard join, digs where
  one asks (`wire::Ask::Dig`) when the cell may be dug, writing the dig to the
  block, and tells each member the world as that member may see it
  (`wire::Board`: only the cells dug, never a gem still buried) as it joins and
  whenever the world changes.
- `HuntRoot` opens as `players::Players` for a Join member, which shows the
  last board the host told it and asks to dig, and as `world::World` for one
  who may read the block, which shows every gem and makes a new world.

Its tests show both sides through `BeuiTest::hosting` and `BeuiTest::joined`
(`hear_member`, `hear_channel`, and `take_block_commands` for what they tell),
and be-host-tests' `a_player_digs_through_the_editor_that_hosts_the_hunt` plays
it end to end: one device hosts while another account's device, given a Join
link, digs through it.

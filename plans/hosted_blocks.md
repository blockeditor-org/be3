# Hosted blocks

Depends on: plans/headless_apps.md for step 4's headless host only. Join links are plans/share_links.md's step 3, which waits on
step 3 here.

A hosted block is run by a host: one member with Edit, online, holding the
block's live session, that others talk to instead of reading the block. A game
world is the first: the block holds the whole world, secrets included; players
see only what the host sends them and act only by asking it. The host is any
member with Edit (decisions/a_hosted_block_is_hosted_by_any_editor.md): share
a world with one editor and two players, and whichever editor is online hosts
while the players join. Chat run by a bot, a private poll or an exam that hides
its answers are the same shape.

## Access

A new level between `KnowExists` and `View`:

| Level | The stored block | The host |
|---|---|---|
| KnowExists | id and type (the name is sealed metadata) | nothing |
| **Join** | nothing: no keys, no objects | a private channel to it |
| View | all of it | a private channel too, and the whole live session |
| Edit | change it directly | may be it |
| Manage | grants and settings | may be it |

Edit means "may change the stored block and so may host it". What someone may
do inside the world (move a piece, kick a player) is the app's own role,
kept in the block's content and checked by the host; a powerful player is a
Join member with a role, not an editor.

### The share dialog

A block type that is hosted says so in its manifest entry, with which levels it
offers and, for each, a name and one line saying what it means. A type that is
not hosted never offers Join; a folder's share dialog shows the core names.

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

- **The host is an account.** The server says which account holds the lease,
  and the joiner is served that account's proof path (guides/blocks/keys.md,
  "Proof paths"): its grant, its authorship or an owner record, at Edit
  or above. The host's static key is the X25519 identity key that record names,
  so only that account's devices can complete the handshake, and the server
  cannot stand in a host of its own or pass the lease to one without Edit. No
  key is derived from the block's data key: anyone who could read the block
  could compute that one, a View member included.
- **Handshake.** Noise IK: the joiner knows the host's static key from its
  checked record and sends a fresh ephemeral key and its account's static key
  (the X25519 identity key its grant names). The prologue is the block id and
  the log head the host's proof was checked at, so both sides agree on which
  block and which state of its access they speak under. The host admits the
  joiner when its grant checks
  (Join or above, guides/blocks/keys.md, "Access logs") and it is not banned. Every later
  message is sealed under the session's keys with its own nonce, so the server
  can neither read, forge, reorder nor replay one. A host change or reconnect
  starts a new handshake.
- **The server routes.** A Join member joins the block's session as a member
  of a new kind: it receives only what the host addresses to it, its messages
  go only to the lease holder, its presence is shown only to the host, and it
  learns whether a host is online and which account it is. View members join as today, and their
  messages to the host use the same channel, so one View member cannot read
  another member's messages to the host. The channel is the session send
  `host` (guides/blocks/server.md, "Live sessions"), the first the server
  checks against the lease: one from anyone but the lease holder naming a
  member, or from the holder naming no member, is refused. What each member
  sends the holder is rate limited by the server. A Join member admitted by a link may message the host though
  the two are not contacts: the link was the out-of-band start
  (decisions/share_links_are_an_out_of_band_start.md).
- The same handshake works over a direct connection a game sets up later; the
  relay is only the first transport.

## Hosting

- Whoever holds the lease hosts; other editors stand by, as terminal hosts do
  (guides/terminal.md). An editor who is not hosting may play through the host
  like anyone else or change the block directly; the app decides which its
  editor offers.
- When the host leaves, another editor takes the lease and the joiners
  reconnect to it. The world continues from what is stored, so a host writes
  its state to the block as it goes; only what it held in memory is lost.
- Banning: a signed **ban record** in the access log (account, signer with
  Edit), which the host refuses at the handshake and the server refuses at
  joining. Any Edit holder may ban: an editor can write anything to the block
  and host with its own logic, so a ban never binds editors, only who joins
  through an honest host or server. A ban and a grant to the same account are
  ordered by their place in the log: a grant signed after a ban (by Manage, as
  every grant is) lifts it, and a ban after a grant bars the account while the
  grant stands. The host and each joiner are served the bans that name that
  joiner, on its proof path.
- Since the host is checked as an account, the app knows which account is
  hosting and may show it
  (decisions/share_links_are_an_out_of_band_start.md).
- **The host component** (its manifest entry, opening and refusals are
  built: guides/apps/host_services.md, "Host components"). The type module stays as simple as text's: storage
  and basic operations on the world, nothing game-specific, since every
  editor runs it as the replica. The rules (what each player sees, which
  moves are allowed, timers) live in the app, in a host component: an entry
  point of the same app wasm with no frame, attached to the replica as an
  editor with Edit is, that hears a member join (account, level, how its
  grant was checked), its messages and its leaving, and sends to one member
  or to several. The Join side is an ordinary editor with no content frame,
  only the channel and whether a host is online. Neither side sees keys, and
  the protocol names no framework types (guides/apps/host_services.md).
- **When a device hosts.** Opening the block in an editor on a device with
  Edit, while no one hosts, starts the host component and takes the lease;
  closing the last editor of that block on the device stops it and gives the
  lease up, so a world left closed uses nothing. Hosting on after the editor
  closes is explicit: a "Keep hosting" action in the frame menu, after which
  the shell lists the block among what this device hosts, with Stop. The
  list is kept in memory only, not in settings or view state: quitting
  be-host stops every host, and starting it again restarts none. On a
  server, `be BLOCK host` runs the same component headless
  (plans/headless_apps.md).

## Accepted limits

- A Join member checks the host on a proof path, which can hide a later
  revocation of the host's Edit behind a hash (guides/blocks/keys.md, "Accepted
  limits"). A server that hides it can keep routing to a revoked editor,
  who then reads the players' moves; it reads nothing stored, having lost its
  keys at the rotation.
- A leaked recovery phrase lets its holder host or join as the account
  (guides/blocks/keys.md, "Accepted limits").

## Order of work

1. **The level** (done). `Access::Join`, granted like any level; Join members see the
   block in Shared with me by its type, as `KnowExists` does (its name is
   sealed metadata they hold no key to; the Join side shows what the host
   tells it), and are served no keys or objects. Like `KnowExists`, a Join
   grant reaches only its own block, never the blocks below it. Done: be-block-graph's random-graph access tests include Join; a
   be-server test that a Join member is served nothing a viewer is
   (`a_join_member_is_served_nothing_a_viewer_is`).
2. **Routing** (done). Session member kinds (`member-kind`), the `host` session
   send, host online state (`session-event.host`, `joined-session.host`),
   rate limits (`limits::HOST_MESSAGES_PER_SECOND`). Done: a test where a Join member's message reaches only the
   lease holder, and another Join member hears nothing of it, not even that it
   was sent (`a_join_members_message_reaches_only_the_host`).
3. **The handshake** (done). Checking the host's account, Noise IK in
   `be-block-keys` (`noise`), the client sides in `be-block-client`
   (`Peer::join_host`, `Peer::admit`). Done: a test with a
   forging server that names an account without Edit as the host, or offers a
   key of its own, and the Join member refuses it
   (`a_join_member_refuses_a_host_the_server_makes_up`); and one where a joiner
   whose grant was revoked is refused
   (`a_joiner_whose_grant_was_revoked_is_refused_by_the_host`).
   Where it differs from the above: a Join member holds no key of the block,
   so nothing carries it the log's root; it takes the root of the log as
   served once a grant to it there is signed by a contact it verified, whose
   hash chain fixes the root (guides/blocks/keys.md, "Accepted limits"), and
   pins it weakly, in memory only. A share link (plans/share_links.md, step 3)
   carries the root instead. Access from above counts: an editor whose Edit
   comes from a block above may host, and a member with View or above from
   anywhere up the path is admitted, but Join itself never passes down. The
   host counts a member as admitted once its first channel message opens (key
   confirmation, since IK's first message can be replayed), and a channel
   message out of order breaks the channel, after which the member handshakes
   again.
   What step 4's host component must do with these: drop a member's channel
   when the member leaves the session (`session-event.joiner`, or a `left`
   membership entry), check every admitted member against the signed logs
   again on each access change (as `Peer::admit` checks it) and drop one that
   no longer holds Join, and make a participant that falls to Join (the server
   takes it out and it joins again as a Join member) handshake again.
4. **Hosting.** The host component, starting and stopping it with the editor,
   Keep hosting and the shell's list, lease handoff and reconnecting, ban
   records; then `be BLOCK host`. In reviewed pieces:
   - a. Ban records (done): `Record::Ban`, `append-log(ban)`, `Peer::ban`;
     `a_banned_member_is_refused_until_a_grant_lifts_the_ban`.
   - b. The host side in be-host (done): the host component runs while an
     editor is open here, the account may edit and this device's replica
     holds the lease (`crates/be-host/src/hosting.rs`), so the lease passes
     to the next editor by the session's order; be-host-stack admits members
     and drops their channels on leaving (`joiner.left`, a `left` membership
     entry), on a message that does not open (taken then as the member's new
     handshake), when the component falls `EVENTS_KEPT` behind, and when a
     check against the signed logs fails, run on `KeysChanged`, `Roots`,
     `TreeAdvanced`, a resynchronized connection, and after this device's own
     change of access or ban (`crates/be-host-stack/src/hosting.rs`); the
     component hears `editor-event.member` and answers with `tell-member`
     (guides/apps/host_services.md, "Host components"), and one that starts
     hears every admitted member join again;
     `a_member_is_admitted_hears_its_host_and_goes_when_revoked`,
     `a_member_that_handshakes_again_is_admitted_again_and_goes_when_banned`.
     Known gap, fixed in d: a device that holds a hosted block's replica open
     for another reason (a child, a preview) holds its lease and runs no host
     component; d claims a hosted type's lease only while a component runs.
   - c. The Join side: an editor opened by a Join member with no content
     frame (`opening-kind.join`), its channel and whether a host is online,
     handshaking again on a host change, and a participant that falls to Join
     handshaking again.
   - d. Keep hosting in the frame menu and the device's list of what it hosts,
     with Stop, in be-host's organization menu.
   - e. `be BLOCK host` (plans/headless_apps.md, step 2).
   Done: a be-host-tests test with two editor devices and two players as raw
   peers, where the first host closes the world and the second carries on;
   and a be-host-stack test where Keep hosting outlives the editor and Stop
   ends it.
5. **The share dialog.** The offered levels, names and lines from the
   manifest (above); who is hosting now. Done: paintings of a game's dialog
   (Join, Host, Manage) and a text block's (unchanged).
6. **A first hosted type**: a small game world in the logic game's spirit,
   with secret state, or a hosted variant of an existing game. Done: a host
   test where an editor hosts and a Join member plays.

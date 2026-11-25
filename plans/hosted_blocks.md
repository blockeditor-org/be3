# Hosted blocks

Depends on: none. Join links are built (guides/blocks/keys.md, "Share links") and `be`
runs host components headless (guides/apps/runtime.md, "Headless").

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
  (guides/apps/runtime.md, "Headless").

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
   pins it weakly, in memory only. A Join member through a share link takes
   it by the grant it signed itself instead (guides/blocks/keys.md, "Share
   links"). Access from above counts: an editor whose Edit
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
4. **Hosting** (done). The host component, starting and stopping it with the editor,
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
     Known gap, fixed in d2: a device that holds a hosted block's replica open
     for another reason (a child, a preview) holds its lease and runs no host
     component; d claims a hosted type's lease only while a component runs.
   - c. The Join side (done): an editor opened on a block its account may
     only join opens with `opening-kind.join`, no content, and the block's
     channel (`editor-event.channel`, `tell-host`); the stack joins the session
     as a Join member and handshakes again on a host change, a rejoin or a
     message that does not open (`crates/be-host-stack/src/joining.rs`); an
     editor whose access crosses between Join and View is closed at the app and
     opened again as the other kind, so a participant that falls to Join
     handshakes again; `a_joined_channel_opens_to_its_host_and_again_to_the_next`.
     View members still reach the host only by a raw `send-host`: an editor
     with View or Edit that is not hosting gets no channel of its own yet.
     Tested: the channel opening, carrying messages in order, made again after
     a message that does not open (a stray one of the old channel leaving the
     new handshake waiting), and with the next host
     (`a_joined_channel_opens_to_its_host_and_again_to_the_next`); an editor
     reopened across Join and View
     (`an_editor_whose_access_crosses_join_opens_again_as_the_other_kind`, its
     access set by a test hook, since no test account can lower another's).
     Untested: a send held back by `slow-down` and sent again; a join that waits
     for a replica of the block to leave; a host that answers the newest of two
     handshakes a member sent while the first was checked; be-sdk's Join opening
     and be-sdk-beui's `on_channel` and `tell_host` in an app.
   - d1. Keep hosting in the frame menu and the device's list of what it hosts,
     with Stop, in be-host's host menu (done): `editor-event.hosting`,
     `block-command.keep-hosting`, `Hosts::keep`;
     `keep_hosting_is_in_the_frame_menu_while_this_device_hosts_the_block`,
     `the_menu_lists_what_this_device_keeps_hosting_with_stop`, and the host
     test `keep_hosting_outlives_the_editor_until_stop`: two devices, the kept
     one hosting on with no editor open and through a reconnect (it holds the
     block's replica open, `be_host_stack::open`, so the lease comes back), until
     Stop. A kept block is let go only on Stop or once the account may no
     longer edit it.
   - d2 (done). Hosting only where a component runs, so a device that holds the
     replica open for another reason (a child, a preview) is never taken for
     the host. The session's lease (`be-block-session`'s `holder`: the editor
     that claimed last, else the first editor) decides who seals the journal,
     and so far also who hosts. The two part:
     - **Sealing does not move.** The holder stays as it is, so every session,
       hosted or not, keeps a sealer whenever an editor is in it, and a
       holder's edits pass to the next holder as today
       (`a_holder_cut_off_before_it_seals_leaves_its_edits_to_the_next_holder`).
     - **The host is the claimed holder only**: the editor that claimed last,
       or no one when no editor in the session claimed. The server's
       `session-event.host`, `joined-session.host` and the `host` send's route
       (`SessionRegistry::host_route`) name it, so a session with no claim has
       no host online, and members wait rather than handshake with a device
       that runs no component.
     - **A device claims when its component starts.** be-host starts a
       component where an editor is open, the account may edit, and no one
       hosts or this device does (no longer "holds the lease"); starting it
       claims the session (`Call::Claim`'s path), which makes this device the
       host and, by the same claim, the sealer. Two devices that start at once
       both claim; the later claim wins, and the other, hearing another host,
       stops its component. A component that stops while the device stays in
       the session (a preview still open) has its replica join the session
       again, which drops the claim; one that stops with the last editor
       drops it by leaving.
     - be-host-stack's `Seen::hosted` follows `session-event.host` naming this
       connection, rather than the replica's holder status.
     Done: `a_device_hosts_only_once_its_component_claims_the_lease`, a claim
     made just before another device's and the stale claim that leaves once
     that device goes included. Left: a claim the server refuses leaves the
     component running unhosted, with no Keep hosting offered, until the block
     stops being one this device should host.
   - e. `be BLOCK host` (done; guides/apps/runtime.md, "Headless"): the type's host
     component headless, while no other device hosts the block;
     `be_hosts_a_block_while_no_other_device_does`.
   Done: a be-host-tests test with two editor devices and two players as raw
   peers, where the first host closes the world and the second carries on
   (`players_carry_on_with_the_next_host_once_the_first_closes_the_world`:
   each player is answered by the first host, then hears the second and is
   answered by it); and Keep hosting outliving the editor until Stop
   (`keep_hosting_outlives_the_editor_until_stop`). Untested end to end: what
   the second host's component hears of the players (the counter's ignores
   them; step 6's game will not).
5. **The share dialog.**
   - a. The offered levels, names and lines from the manifest (above) (done):
     a host component's `levels` (`HostManifest::levels`, Join required, on a
     type of its own app only, one host per type across apps), the access
     listing's `levels`, and be-sdk-shell's `Offered`, which every level choice
     and name in the dialog goes through, listed by level, with the chosen
     level's line under its choice; people added, contact links and share links
     start at Join. `the_share_dialog_makes_a_join_link_on_a_hosted_block` paints
     a game's dialog: Join game on the link with its line, Ada's row at Host,
     mine at Manage; a text block's paintings are unchanged.
   - b. Who is hosting now, in the dialog (done): the access list names the
     account holding the lease of the block's live session (`access-list.host`,
     `access-listing.host`), and a hosted block's dialog says who that is, or
     that no one hosts it, under "People with access"; member notes use the
     type's names too. `the_access_list_names_who_hosts_the_block_now`; the
     game's painting shows "Ada is hosting it now." As of listing: the dialog
     lists again on each change it makes, not when the lease moves meanwhile.
6. **A first hosted type**: a small game world in the logic game's spirit,
   with secret state, or a hosted variant of an existing game. Done: a host
   test where an editor hosts and a Join member plays.

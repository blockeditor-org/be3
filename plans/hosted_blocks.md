# Hosted blocks

Depends on: plans/session_message_authority.md, plans/signed_access.md;
plans/share_links.md for step 6 only.

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
| KnowExists | id, type, name | whether one is online |
| **Join** | nothing: no keys, no objects | a private channel to it |
| View | all of it | a private channel too, and the whole live session |
| Edit | change it directly | may be it |
| Manage | grants and settings | may be it |

Edit means "may change the stored block and so may host it". What someone may
do inside the world (move a piece, kick a player) is the app's own role,
kept in the block's content and checked by the host; a powerful player is a
Join member with a role, not an editor.

A block type that is hosted says so, with its own name and one line for each
level, in its manifest entry; the share dialog shows them (a game: "Play",
"Spectate: sees everything", "Host and edit the world"). A type that is not
hosted never offers Join.

## The channel

- **Host key.** An X25519 key pair derived from the block's current data key
  (HKDF, its own domain), so it changes with every rotation and anyone who can
  read the block could compute it; the server lets only Edit take the lease.
  When a data key is made, a member with Edit signs a **host key record**
  (block, key epoch, public key) into the block's access log
  (plans/signed_access.md), and a Join member checks it back to the block's
  root, so the server cannot stand in a host of its own.
- **Handshake.** Noise IK: the joiner knows the host key from the record,
  sends a fresh ephemeral key and its account's static key, and signs the
  handshake hash with its signing key. A joiner through a link mixes in the
  link secret as a pre-shared key. The host admits it when its grant checks
  (Join or above, plans/signed_access.md) and it is not banned. Every later
  message is sealed under the session's keys with its own nonce, so the server
  can neither read, forge, reorder nor replay one. A host change or reconnect
  starts a new handshake.
- **The server routes.** A Join member joins the block's session as a member
  of a new kind: it receives only what the host addresses to it, its messages
  go only to the lease holder, its presence is shown only to the host, and it
  learns only whether a host is online. View members join as today, and their
  messages to the host use the same channel, so one spectator cannot read a
  player's moves. A relay of the host kind from anyone but the lease holder,
  or to a member of another kind, is refused (the check
  plans/session_message_authority.md adds, by relay kind). Each Join member
  is rate limited by the server.
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
- Banning: a signed **ban record** in the access log (account, block, signer),
  which the host refuses at the handshake and the server refuses at joining.
- Apps: the host side hears a member join (account, level, how its grant was
  checked), its messages, and its leaving, and sends to one member or to
  several; the Join side has an editor with no content frame, only the
  channel and whether a host is online. Neither side sees keys. The protocol
  names no framework types (guides/apps/host_services.md).

## Order of work

1. **The level.** `Access::Join`, granted like any level; Join members see the
   block in Shared with me with its name and type and are served no keys or
   objects. Done: be-block-graph's random-graph access tests include Join; a
   be-server test that a Join member is served nothing a viewer is.
2. **Routing.** Session member kinds, the host relay kind, host online state,
   rate limits. Done: a test where a Join member's message reaches only the
   lease holder, and another Join member hears nothing of it, not even that it
   was sent.
3. **Host keys and the handshake.** Host key records, Noise IK in
   `be-block-keys`, the client sides in `be-block-client`. Done: a test with a
   forging server that offers its own host key to a Join member, which refuses
   it; and one where a joiner whose grant was revoked is refused.
4. **Hosting.** Lease handoff and reconnecting, ban records, the app side
   (Q9 decides where). Done: a test with two editors and two
   players where the first host leaves and the second carries on.
5. **The share dialog.** Labels and lines from the manifest; Join offered only
   for hosted types; who is hosting now. Done: paintings.
6. **Join links** (plans/share_links.md, step 3).
7. **A first hosted type**: a small game world in the logic game's spirit,
   with secret state, or a hosted variant of an existing game. Done: a host
   test where an editor hosts and a Join member plays.

## Open questions

- **Q7. The name.** Is "Join" right for the core level, which every type sees
  under its own label? "Participate" and "Use" are the alternatives.
- **Q8. Who bans.** Any Edit holder, since any of them may host and must be
  able to protect the session it runs, or Manage only?
- **Q9. Where the host's logic runs.** In the block's type module, on the
  replica that owns the session (it then runs in be-host's worker with no
  editor open, and in `be BLOCK host` headless on a server, as the terminal's
  host does), or in the app's editor, which hosts only while it is open?
- **Q10. Does a joiner learn who hosts.** The host key is enough for the
  channel, so the host's account can stay hidden from Join members (they learn
  only "a host is online"), or the host can sign the handshake with its
  account key so players see who is running the world. Hidden by default?
- **Q11. Edit holders without the host's logic.** If an editor's build of the
  type module is older than the host's, should it refuse to host (as terminal
  hosts require one build), or may the lease go to any editor?

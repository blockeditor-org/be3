# Connected apps

Depends on: plans/unfiled.md, and a versioned server protocol, which no
plan covers yet.

A third party's own product (its own website, login and servers) that connects
to a person's be3 account and reads and edits their blocks. Until block apps
are established, a company can't justify an app that works only inside
be-host, and its users should still keep their documents in their own account.

Nothing here is built. It waits for the server protocol to be frozen:
today the server refuses any client of another build
(`decisions/server_protocol_is_unversioned.md`), so no outside client can keep
up with it. This plan is what to build once that changes.

## Goals

- Works with any server, never a server or URL of ours: custom servers are
  always supported.
- No registration and no involvement from us: anyone can publish an app. The
  permission boundaries carry the safety.
- A third party never holds an account's identity secret or its scope keys.
- Blocks an app makes, or blocks of its types the person makes anywhere, are
  reachable wherever they are in the tree; no app depends on where a file sits,
  except the account's `Settings` block.
- No app folder or anything else added to a person's workspaces.

## Trust tiers

One rule covers every app: an app that can reach the network can leak exactly
what it can open, wherever it runs.

- **Inside be-host, without network access** (open source apps, ideally): an
  app never sees keys and its data never leaves the device, so it needs no
  grants and appears in no share menu.
- **Inside be-host, with network access**: the reach rules below apply to it as
  they do to a connected app. How an app declares and is limited to its network
  access is decided later, when wasm apps are made safe.
- **Outside be-host** (a connected app): the reach rules below, plus a key of
  its own.

Legitimate companies will mostly run outside be-host; running inside it is
better and a hard sell. A company that later moves inside keeps its grants.

## Publishers own their block types

There is no registry, so an app's identity proves itself:

- An app has a publisher signing key. A third-party block type's id is derived
  from it, `hash(publisher key, name)`, so nobody can declare another
  publisher's type (today a type id is any UUID a manifest picks, so an app
  could claim the text type). First-party types keep their UUIDs, which frozen
  formats store, and belong to the be3 project.
- The person's device, not the server, fetches
  `https://DOMAIN/.well-known/be3-app.json`, which names the publisher key, and
  shows the domain as verified; an app without one is shown as unverified.
- A replaced publisher key is signed by the one before it, as escrow keys are
  chained.
- A type's format is its type module, which belongs to the publisher. Anyone
  may write another editor for a type: it loads the owner's type module by
  hash, so two editors of one type share live sessions.

A publisher that is sold or compromised reaches every block of its types. That
is the risk of any app that relies on outside services, and the consent screen
says so.

## What a connection reaches

Connecting an app grants nothing by itself. A connection acts as the person:
what it makes is theirs (commits record the connection, and presence shows
"Ada (Logicgame)"), and it reaches a block when the person can view it and
either:

- **its publisher owns the block's type.** Ada made a level with Logicgame, so
  if she shares it, the person's Logicgame opens it; Ada never has to say so.
  A new app owns no existing types, so by default it reaches only what its own
  software made, which is what keeps open publishing safe. The app's own
  settings kinds are owned types too.
- **an app grant on the block or above it names the publisher.** An app grant
  is a share whose recipient is a publisher instead of a person: made by
  someone with Manage, inherited down the tree like any grant
  (`decisions/block_access_is_the_highest_grant_on_the_path_up.md`), at most
  Edit. A website builder needs every block under a site's folder: Ben grants
  it on the folder, shares the workspace with Cara, and Cara's Website builder
  edits the site with no setup. This draws the line sharing already draws:
  whoever manages a folder decides who reaches what's in it, including what
  other members wrote there.

A grant may name a type instead of a subtree (Canvas Pro editing canvases from
another publisher), and covers only blocks the granter manages. Ada's canvas
shared with Ben stays closed to Ben's Canvas Pro until Ada grants it; a
"Ben wants to open this with Canvas Pro" request makes that one step. A company
can grant an app on a workspace or the organization.

The server enforces this: it knows every block's type and the grants. The
connection's token can't share, manage contacts, invite, change membership or
organizations, pair, or set a recovery phrase. Sharing from inside an app is a
link to be-host.

## Keys

- **The grant key.** The app makes its own key pair. On consent, the person's
  device makes a grant key G for the connection, seals it to the app's verified
  key, and keeps it itself (`RecoverySecret::seal_own`). The connection's record
  (app key, publisher, G) is sealed the same way, so devices ignore one the
  server made up or widened.
- **Owned types.** Each data key of a block the connection reaches by type is
  also wrapped under G. A device wraps at creation when a connection covers the
  type, and otherwise through key work: `GetKeyWork` lists reachable blocks
  with no wrap under G, and `Peer::tend` wraps them, as it seals `unsealed`
  scope keys today. A block someone else made becomes readable once one of the
  person's devices has been online. `PutKeys` must accept these wraps on blocks
  the person may only view.
- **App grants.** The person's own devices already hold the granted folder's
  scope key, so they seal it to their own G; nobody else has to be online.
  Devices trust the server about grants and roles today, so a forged app grant
  would hand keys to an app the person connected; signing app grants with the
  granter's identity key, checked against verified contacts, closes that.
- **Rotation.** A device moving a block to a new key epoch wraps it under every
  G that reaches it.
- **Ending access.** Disconnecting, removing a grant or blocking an app drops
  the G wraps and the app's sessions, and records rotations for the blocks it
  reached. Rotation is lazy (content isn't resealed), but costs one per block.

## Connecting

1. The app asks which server the account is on. A server describes itself at
   `{server}/.well-known/be3`: its API and its web client, if it hosts one.
2. The app makes its key pair and posts a connect request to that server, its
   details sealed with a code, as a contact link is (guides/blocks/keys.md).
3. It sends the person to their own be-host, the only place that holds their
   keys: a `be3:` link a native host claims, the server's web client, or a code
   typed in. The code stays in the link, never on the server.
4. be-host verifies the domain, shows the publisher, what it will reach and the
   risk, and on approval creates the connection and returns the token and G
   sealed with the code.

Connecting again replaces the connection. One connection is per person and
publisher; sharing its secret across the app's own clients or servers is the
app's business, and the consent screen can't verify where keys live.

The app links the client library (be-block-client and the type host) at the
frozen protocol version, as it would any library. It never loads code from us
or any other origin at runtime.

## Making a file: Unfiled

An app creates a block of its type in the account's Unfiled
(plans/unfiled.md), never learning the root's id, and finds its blocks by id or
by listing its types. The person files them later with a normal move (rekeying
into the destination scope).

- **Keys.** The app can't wrap under Unfiled's scope key, so it wraps the new
  data key under G only; the person's devices open G and rewrap under the scope
  key through key work.

## Listing

A new listing answers the blocks of given types the account can view, across
the graph. A connection sees names (sealed with keys it holds) but not folder
names, so an app shows a flat or recent list.

## In be-host

- **The share menu** lists who outside the person's devices can read a block:
  people, connected apps, and in-host apps with network access, such as
  "Logicgame, through you" or "Website builder, through Ben". Revoking opens a
  modal explaining the access:
  - through type ownership: block connected apps on this block (Manage; the
    server enforces it and the block rotates, so even the app that made it can
    no longer open it), or disconnect the app from the account;
  - through an app grant: remove the grant, rotating what it covered.
- **Settings** lists connections, with Disconnect.
- **Organizations** can turn connected apps off, or allow only some, as a
  server-enforced policy.
- **Foreign types.** A publisher's `.well-known` file names its types, icons and
  an open URL, so be-host lists those blocks by name with "Open in APP" even
  without the app installed.
- **Access requests** ("Ben wants to open this with Canvas Pro") reach the
  block's managers.

## Rejected

- **A hosted SDK script** (be3 serving the client for apps to load): it ties
  apps to one server, and loading code from another origin is an XSS hole. It
  only existed to dodge the build-id lock, which the protocol freeze removes.
- **An app folder per app**, and app-private data roots: they clutter
  workspaces and tie apps to positions in the tree. Unfiled replaces both.
- **Connected apps as agents** (separate accounts that see only what is shared
  with them): an agent is someone else, while a connected app acts as the
  person, and needs to reach its types wherever they are. Agents stay as they
  are.
- **A connection reaching everything the person can view**: the people who
  shared those blocks never agreed to that app.
- **Publishers endorsing other publishers' editors** for their types: it lets a
  publisher pass on access its users gave it. Use an app grant instead.

## Open

- Whether type ids derived from a publisher key fit the frozen formats that
  name types, and how first-party types are listed as be3's.
- Signing app grants (Keys, above).
- How an in-host app declares network access, decided with wasm app safety.

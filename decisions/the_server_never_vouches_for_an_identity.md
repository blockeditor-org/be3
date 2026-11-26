# The server never vouches for an identity

No device trusts a person, a key or a right because the server said so. Who
someone is comes from a contact link, a link's secret or a key change sealed
with a verified key; what they may do comes from records signed by members
(guides/blocks/keys.md, "Access logs"). Given `None` or `KnowExists` on a block, the server
may withhold, delay or destroy it, never read or write it, alone or with
anyone the block's managers did not choose. Given View, it may also write,
since Edit is enforced by the server alone.

## Standing exceptions

What the server still decides, and devices or other servers' checks take on
its word:

- **The organization settings key.** The settings block is shared with the
  server's own account so it can enforce policies
  (decisions/server_enforced_settings_are_shared_with_the_server.md); devices
  seal it to the key the server names as its own (`ServerKey`).
- **Membership.** Who is a member or guest of an organization, and so who may
  be granted there or see it listed. Administrators are signed; membership is
  not.
- **Link policy.** Who may open a share link (guides/blocks/keys.md, "Share links") is a
  setting the server enforces: a dishonest server can let anyone with the
  secret open it.
- **Edit over View.** They hold the same keys, so a server granted View can
  write.
- **The host lease.** Which editor hosts a hosted block
  (guides/blocks/hosted.md): devices check that the host has Edit, but not
  which editor the server picked.
- **Freshness.** Which log head and key epoch are newest, short of a fork
  (guides/blocks/keys.md, "Accepted limits").

## Why

The server stores and relays everything. If it could introduce a person or a
right that devices act on, it could make up a member, a host or a grantee and
read whatever they would be sent, and nothing else in the key design would
matter.

## Rejected

- The server vouching for a host or a player in a hosted block
  (guides/blocks/hosted.md): it could make up a host and read every player's
  moves, or a player and see the world.
- Grants as server rows that devices seal to: the server could name any real
  contact as granted (guides/blocks/keys.md, "What the server can and cannot
  do"), which the signed access logs replaced.
- Separate keys for Edit and View: it would stop a server with View from
  writing, which is accepted, at the cost of a second key per scope and its
  rotation.

# The server never vouches for an identity

No device trusts a person, a key or a right because the server said so. Who
someone is comes from a contact link, a link's secret or a key change sealed
with a verified key; what they may do comes from records signed by members
(plans/signed_access.md). Given `None` or `KnowExists` on a block, the server
may withhold, delay or destroy it, never read or write it, alone or with
anyone the block's managers did not choose. Given View, it may also write,
since Edit is enforced by the server alone.

## Why

The server stores and relays everything. If it could introduce a person or a
right that devices act on, it could make up a member, a host or a grantee and
read whatever they would be sent, and nothing else in the key design would
matter.

## Rejected

- The server vouching for a host or a player in a hosted block
  (plans/hosted_blocks.md): it could make up a host and read every player's
  moves, or a player and see the world.
- Grants as server rows that devices seal to: the server could name any real
  contact as granted (guides/blocks/keys.md, "What the server can and cannot
  do"), which plans/signed_access.md removes.
- Separate keys for Edit and View: it would stop a server with View from
  writing, which is accepted, at the cost of a second key per scope and its
  rotation.

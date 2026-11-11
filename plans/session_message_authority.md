# Session messages from the right participants

Depends on: none.

The server relays a session message (`SessionRelay`, be-server `client.rs`) for any
participant who joined, and joining needs only view access. It does not look at
the payload. So a viewer can relay a `Submit` as if it were an editor, and an
owner's `Accepted`, `Snapshot`, `Sealed` or `Replaced` as if it were the owner.

A follower now heeds those four from the owner alone (`not_from_the_owner` in
`be-sdk-type-guest/src/replica.rs`), but an owner still accepts a `Submit` from
whoever sent it, so a viewer can edit through the session, and a viewer's
message that names a part makes the owner read it.

Make the server refuse it:

- Relay a message to the participants only if its sender may do what it says: a
  session message that carries operations or session state (everything except
  presence and peer channel messages) only from a participant with edit access.
- The server cannot read the payload (it is the replicas' wire format), so the
  split is by call: a relay that carries session state names itself, such as a
  `kind` field on `SessionRelay`, which the replica sets and the server checks
  against the sender's access.
- Check that the sender is the session's owner for the owner-only kinds.

Done: a test where a viewer's `Submit` and `Accepted` reach no replica, and an
editor's do.

# A sequence insert names its first offset and uses it up even when refused

Each client appends everything it inserts in a session to one buffer of its
own, so offsets are unique without coordination. An insert names its first
offset (`start`) and is refused unless it is the next offset of that client's
buffer. An insert or replace with the right `start` uses up its offsets even
when it is refused for another reason (a replace whose text someone else
changed first, an anchor nobody knows). The owner does not check that a client
only appends to its own buffer, so an insert resubmitted after a reconnect can
still name its old client.

## Why

- A wrong `start` never fires in normal use (operations are deduplicated by
  `OpId` and arrive in order per client); if it does, the edit is visibly lost
  instead of two elements sharing an id.
- Using up the offsets keeps the client's later inserts, numbered as if the
  refused one had applied, landing. Without that, losing one replace race lost
  everything the client typed after it.

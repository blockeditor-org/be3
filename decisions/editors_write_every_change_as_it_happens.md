# Editors write every change as it happens

An editor writes each change to its block the moment it makes it - each frame of a drag, a stroke as it grows, a value as it is typed - rather than holding a gesture back and writing it on release. A gesture is grouped for undo with `begin_gesture` and `commit_gesture` (or called off with `cancel_gesture`), not by delaying its writes. This applies to every editor (guides/adding_an_app.md, Gestures).

## Why

Peers see the change through sync at once. Saving is already batched by the client (at most every 750 ms), so writing often does not mean saving often; if live edits ever cost too much, the client is where batching goes.

## Rejected

- Writing a gesture once, on release: peers would not see it until it ends, and batching belongs in the client, which already does it.

# The text app on Android

Depends on: none.

The text app should work on a phone like this:

- open a long Markdown document, type, lock the phone, unlock it;
- swipe back and open another document;
- scroll smoothly without wasting power;
- swipe back and reopen the first document at the same scroll position;
- close and reopen the app, still at that scroll position;
- work offline-first: never lose or duplicate offline edits, open the app and edit with no
  connection, and survive the connection dropping and coming back;
- show no single-frame glitches;
- toggle the keyboard cleanly.

Each item below was found by running be-host headless at phone size
(`./scripts/buck run //crates/be-host:dev -- --size=390x844@3`) and driving it with touch,
swipes and `drive back`. Items marked "from reading the code" were not run, because nothing
here runs Android. Line numbers are from when this was written. Remove an item when it lands.

## Notes for reproducing

- `drive` has no command for lock or unlock, or for keyboard insets. `drive blur` and
  `drive focus` stand in for lock and unlock. `drive resize 390x480@3` stands in for the
  keyboard opening on API 30 and later, where the keyboard's inset shrinks the area be-host
  lays out (`BeuiView.java:90-96`).
- To go offline, block the in-process server's listening port with iptables. The port changes
  on every launch: find it in `/proc/net/tcp` as the local listener with an established
  connection from be-host. Reject both directions with `--reject-with tcp-reset`.
- To start with no connection, block SYNs to the ephemeral ports (`32768:60999`) on `lo`, but
  accept the build daemon's and the build-server proxy's ports first. Blocking those breaks
  `./scripts/buck` (the proxy was 33713 here). Start the build daemon before adding the block.
- `drive back left` (or `right`) starts a gesture and leaves it running until `back commit` or
  `back cancel`. Plain `drive back` is the whole gesture.
- `drive frames` counts frames headless with no vsync, so a fling runs about 220 window frames
  a second. Compare ratios, not rates.

## Offline-first

Host tests reproduce these with a device of a test's server (guides/testing_a_gui.md, 6):
`HostTest::offline` is a drop the device notices at once, `HostTest::unreachable` one it does
not (its connections stay open and carry nothing), which is what blocking the port did. A drop
under a request is noticed once an early ping goes unanswered, about 25 s (`ANSWER_WAIT` plus
`PONG_WAIT`, be-block-client connection.rs), and everything below waits that long.

4. **Starting with no connection.** A device of a remote server restarted while it cannot reach
   it shows Files at once (`resume_organization`); the blank window seen here was probably the
   `:dev` embedded server blocked by the same rule. What is left: My files is empty and the
   document that was open is not reopened, because My files' cached listing is from before the
   device's own new block (the gap fix/offline-my-files works on). `a_device_that_cannot_reach_its_server_opens_its_files_and_edits_at_once` is ignored
   until then.
5. **Create, duplicate, define, versions and merging still wait in the block store's loop.**
   Commands that need only the server run in a lane beside it (guides/blocks/host.md), but these
   also need the open blocks. Moving create there broke the commands that follow it without
   naming the new block (branches, checkouts, imports): split each into its loop part and its
   server part with an order that covers what they depend on.
7. **Why a reconnect hits "the session has a newer owner"** is still not known: a test's drop and
   reconnect did not meet it. The error no longer outlives its cause: it clears once the device is
   connected with nothing waiting (`report` in worker.rs).

What already works, and must keep working:
- an edit made during a drop came through a reconnect once, with no duplicate, and synced;
- edits survive `kill -9`, even straight after typing.

## Lock, unlock and single frames (from reading the code)

16. **The window loses 1 px of scroll when the format bar hides.** On `drive blur` (standing
    in for lock), the format bar goes and every line moves up by 1 px (line 279 from y=136 to
    y=135). Keep the scroll anchored to the content.
    Not reproduced since: with a 300-line Markdown document at several scroll offsets (from
    wheel, drag and fling, a heading under the caret, a keyboard-height window), blur moved no
    line, except within a bar's height of the end, where the view is pinned there. Find the
    document and offset that show it before fixing.

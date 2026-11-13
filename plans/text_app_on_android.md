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

What already works, and must keep working:
- an edit made during a drop came through a reconnect once, with no duplicate, and synced;
- edits survive `kill -9`, even straight after typing.

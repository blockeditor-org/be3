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

4. **Starting with no connection shows a blank window.** With the server unreachable from
   launch, the window stayed empty for more than 30 s, and `drive` reported it settled with an
   empty tree. The workspace, files and the last open document appeared about 10 s after the
   connection was let through. Starting must not wait for the server: open from local data,
   and allow editing.
5. **The files pane hangs on "Loading the files" after the connection drops, and stays stuck
   after reconnecting.**
   - After the server port was blocked, going back from a document showed
     `#editor.loading` "Loading the files" for more than 75 s.
   - It stayed that way for more than 60 s after the socket reconnected, until the app
     restarted.
   - Because going back closes the document's tab, no document can be reached meanwhile.

   The files pane should show the local graph offline and recover on reconnect.
6. **The status during a drop tells offline users to keep the app open.**
   - With the connection blocked, a toast said "Saving changes… This is taking longer than
     usual. Keep be-host open until it finishes."
   - The Block Stack panel said "Changes: 1 not yet saved on this device" for more than a
     minute.
   - The edit did survive a `kill -9`, so it was in the journal, and both messages are wrong.
   - Being offline should read as offline and saved on this device, not as a stalled save.
     Check that the local seal really does not wait on the network.
7. **A sync error outlives its cause.** After a connection reset, the host menu said
   "Changes could not sync" and the Block Stack panel said "Error: the server refused
   (InvalidRequest): the session has a newer owner". Both stayed after a clean restart, with
   "Waiting to sync: 0". They cleared only on a later relaunch. Find:
   - why a reconnect hits "the session has a newer owner";
   - why the error stays once nothing is waiting.
8. **While the connection is down but not yet noticed, the block store stays busy.** Typing
   right after the port was blocked kept `drive` from settling for 30 s ("the block store sends
   sealed edits; … lists a block's revisions; … indexes edits"). Check that nothing the editor
   or files pane waits on is stuck behind a send that cannot finish.

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

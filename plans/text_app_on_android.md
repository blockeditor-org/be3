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

## Scroll position

1. **The text app never saves its scroll position.** Reopening a document after going back,
   or after relaunching the app, shows line 1. be-app-text never calls `set_view_state`, and
   `State::pump` puts the selection at byte 0 on first load
   (`crates/be-app-text/src/root/state.rs:152-159`).
   - Save the scroll position, and probably the caret, so they survive a process kill. Write
     them debounced or when the app goes to the background, not only on a clean exit.
   - Restore them when the document opens.
   - Going back closes the document's tab, and closing a tab detaches its view
     (guides/blocks/workspaces.md). A position kept per tab is therefore lost on every
     back-then-reopen. Decide where it lives so that reopening a document from the files list
     finds it.

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

## Keyboard

11. **The keyboard is not shown again on unlock** (from reading the code). If it was up when
    the phone locked, nothing asks for it again: `View` is only sent from `nativeCreate`, and
    `show_window` acts only on a change (`crates/beui-platform-android/src/lib.rs:258-274`).
12. **The layout jumps in one step while the keyboard slides** (from reading the code). There
    is no `WindowInsetsAnimation` callback, so the clear color can show under the moving
    keyboard. Follow the inset animation.

## Lock, unlock and single frames (from reading the code)

13. **Unlocking can show a blank or stale frame.** `BeuiView` implements only
    `SurfaceHolder.Callback`, and `surfaceChanged` returns before a frame is presented
    (`crates/beui-platform-android/android/java/com/be3/beui/BeuiView.java:53-60`). Implement
    `SurfaceHolder.Callback2.surfaceRedrawNeeded` (or `surfaceRedrawNeededAsync`) so Android
    waits for the first frame on the new surface.
14. **Input that arrives while there is no surface is thrown away**
    (`crates/beui-runner/src/lib.rs:542-544`). A late touch cancel, focus loss or IME event
    can be lost, leaving a stuck press or drag after resume. Keep the events that end
    something.
15. **The surface can be destroyed while the renderer still holds it.** `surfaceDestroyed`
    waits at most 2 s for the runner to let go
    (`crates/beui-platform-android/src/lib.rs:47, 1078-1086`). A frame that runs longer (a
    long wasm step) leaves the renderer holding a destroyed surface.
16. **The window loses 1 px of scroll when the format bar hides.** On `drive blur` (standing
    in for lock), the format bar goes and every line moves up by 1 px (line 279 from y=136 to
    y=135). Keep the scroll anchored to the content.
    Not reproduced since: with a 300-line Markdown document at several scroll offsets (from
    wheel, drag and fling, a heading under the caret, a keyboard-height window), blur moved no
    line, except within a bar's height of the end, where the view is pinned there. Find the
    document and offset that show it before fixing.

## Power and frames

17. **The text app runs about three steps for every host frame.**
    - One fling: 351 window frames, about 1,060 `be3.text` steps (`drive frames`).
    - Typing: 708 window frames, 683 steps.
    - Idle with the caret blinking: 17 window frames, 24 steps.

    guides/apps/runtime.md says a runtime is asked at most once per host frame. Find what
    these steps are (input, layout and present commits, or extra draws) and drop any that
    draw nothing.
18. **A focused editor never goes idle.** With the keyboard up, the host ran about 4 frames a
    second. The caret blink asks for a repaint every 530 ms with no end
    (`crates/beui-core/src/base/text.rs:29, 752-764`). Two frames per blink is the most it
    should cost. Consider stopping the blink after a while with no input, as other platforms
    do.

What already works: a fling decelerates smoothly frame by frame and ends within about 1 s. An
unfocused editor draws nothing while idle.

## Opening documents

19. **Importing a text file gives an empty document.** "New → New in My files… → Import a
    file…" (`drive upload FILE '#picker.import'`) with a `.md` file, even a two-line one,
    opens a Text block with no content.
    - `TextBlock::from_file` looks right (`crates/be-type-text/src/text.rs:227-243`).
    - Start from `be_host_stack::import_file` (`crates/be-host-stack/src/lib.rs:1475`) and
      `Replicas::from_file` (`crates/be-host-stack/src/worker.rs:2639`).
    - `a_picked_file_becomes_a_block_of_the_type_that_claims_it` checks an image and an
      epub, never a text file's content. Add one.
20. **After an import, going back leaves the new file's row renaming, with focus.** The
    files-list row of the imported file was a focused `TextInput "Name"`. On a phone that
    raises the keyboard over the files list.

## Accessibility and small things

21. **The text app's line labels end in a stray "mm"**: `Label value="## Section 32mm"`, every
    paragraph `…keyboard.mm`. List items start with U+FFFC: `Label value="￼item a1mm"`. A
    screen reader reads both. Find where the line's label is built.
22. **The tab switcher's document tab has no accessible name**
    (`Button #dock.switcher.tab.11` with no label, beside its "Close tab" button).
23. **The open-tabs badge disagrees with the switcher.** The badge says "3 open tabs" with only
    Files, Settings and History listed, under "Nothing else is open.". It says "4" with one
    document open.

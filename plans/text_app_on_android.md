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

## Back gesture

2. **The system back gesture stops working after its first use in a session.** The first
   `drive back` after launch leaves the document for Files. Every later one, in any document,
   slides and then snaps back to the document. The dock's Back button (`#dock.back`) still
   works.
   - `BackSlide` gets `Invoked`, calls `on_back`, and returns because the tab is still open
     (`crates/beui-components-unstyled/src/back_slide.rs:121-136`).
   - `on_back` is `dock.go_back()` (`crates/beui-dock-host/src/unstyled.rs:937-947`), which
     does not close or leave the tab in this state.
   - Find why: the dock history (`crates/beui-dock-state/src/history.rs`) or
     `closes_going_back` are the first suspects.
3. **The failing back gesture draws the wrong pages.** Step it frame by frame (`drive clock
   stop`, `drive --no-settle back`, then `pause 16` and `shot` per frame):
   - the page that slides away is a copy of the Files page (back arrow, "Files" title, the
     list), not the document;
   - the page behind it is blank;
   - next comes a frame of Files with an empty body;
   - then it jumps back to the document.

   Check that the slide shows the document over the Files preview, including once item 2 is
   fixed. Add a painting of a mid-gesture frame.

## Offline-first

Host tests reproduce these with a device of a test's server (guides/testing_a_gui.md, 6):
`HostTest::offline` is a drop the device notices at once, `HostTest::unreachable` one it does
not (its connections stay open and carry nothing), which is what blocking the port did. A drop
not noticed lasts until the connection's ping goes unanswered, about 35 s (`PING_INTERVAL` plus
`PONG_WAIT`, be-block-client connection.rs), and everything below waits that long.

4. **Starting with no connection.** A device of a remote server restarted while it cannot reach
   it shows Files at once (`resume_organization`); the blank window seen here was probably the
   `:dev` embedded server blocked by the same rule. What is left: My files is empty and the
   document that was open is not reopened, because My files' cached listing is from before the
   device's own new block (the gap fix/offline-my-files works on). `a_device_that_cannot_reach_its_server_opens_its_files_and_edits_at_once` is ignored
   until then.
5. **The files pane, and every command, wait on one that needs the server.** Going back on a phone
   runs a `SetParent`, which waits on the server in the block store's loop; every command queued
   behind it waits too (the busy reason names the step: "the block store's loop is busy with set
   parent"). guides/blocks/host.md lists this as not built: run commands that need the server off
   the loop, ordered per block (`Command::needs`), as uploads are.
   `on_a_phone_the_files_show_what_the_device_keeps_after_its_connection_drops` is ignored until
   then.
6. **Edits typed while a seal waits on the server are not kept.** With sealing at its real pace
   (`drive hurry off`), the text module's publish waits on a server that stopped answering; while
   that seal is out the module holds new operations (`held`, be-sdk-type-guest replica.rs,
   `sealing_out`) out of the journal. So the block store counts them as not saved on this device
   (the "Saving changes" toast), and the module stays unsettled, which stops even `drive pause`.
   Either release a publish unanswered for a moment as kept, a local commit the outbox sends, as
   `Replicas::keep_all` does at a flush, or journal what is typed during a seal.
   `an_edit_made_offline_reads_as_kept_on_the_device_and_syncs_without_an_error` is ignored until
   then.
7. **Why a reconnect hits "the session has a newer owner"** is still not known: a test's drop and
   reconnect did not meet it. The error no longer outlives its cause: it clears once the device is
   connected with nothing waiting (`report` in worker.rs).
8. **Search indexing waits on the server.** Typing as the connection silently drops keeps the host
   busy with "the block store indexes edits or answers a search" until the drop is noticed.
   Sending the outbox and listing revisions are unsynced now, not busy, and a flush keeps on the
   device without waiting for the server, so closing no longer waits on it.
   `typing_as_the_connection_silently_drops_leaves_the_host_settled` is ignored until then.

What already works, and must keep working:
- an edit made during a drop came through a reconnect once, with no duplicate, and synced;
- edits survive `kill -9`, even straight after typing.

## Keyboard

9. **The caret is not revealed when the keyboard opens.** Tap a line near the bottom of a long
   document, then shrink the window to the keyboard-up height. The caret stays under the
   keyboard until you type.
   - The host's `reveal_focus` reveals the focused node, not the caret
     (`crates/beui-core/src/document.rs:1536-1539`).
   - TextArea reveals the caret only on edits and keys
     (`crates/beui-components-unstyled/src/text_area.rs:454-505`).

   Reveal the caret when the viewport shrinks.
10. **The editor reports the whole text field as the caret to the input method after a
    resize.** `drive state` says `its caret at 0,-800 size 1170x61905` until the next edit. It
    should stay the caret's rectangle. Input methods use it to place their UI.
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

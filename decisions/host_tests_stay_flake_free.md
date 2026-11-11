# Host tests stay flake free by what the tools wait for, and a stress run finds the rest

A host test fails under load when it reads something the machine has not finished with: an
app's answer on a thread of its own, a server's push, what a resting pointer hovers. These
rules make that hard to write and easy to find.

- Already landed in #837: `drive pause` runs each frame of a stopped clock only once the
  program has nothing to wait for (`Session::quiet`), and a frame asked of an app while another
  is in flight is asked again once that one is answered (`Pacing::need`). #845 made a long
  pause take a bounded number of frames, since each frame is now a round trip to the apps.
- Already there: `drive paint` and every input command wait for the program to be quiet, and
  fail after 30 seconds saying what kept it busy.
- `HostTest::record` paints the frame twice with a settle between and fails with "the painting
  is not settled" if the two differ. It catches a hover, an animation's last step or an app's
  late frame in any test, whoever wrote it, the first time it shows. It has no opt-out; a
  painting that changes between two frames of a program with nothing to do is not a painting.
- `./scripts/buck run //crates/be-tool-local:bin -- stress NAME` runs host tests many times at
  once (twice the cores by default), kills a run that hangs, and counts the failures by message
  with the first logs of each, so a flake shows before it reaches a pull request. It is local
  and on demand, and it runs every test against the host that has every app, where
  `./scripts/test` stages the apps a test names.
- The rules for writing a test live in guides/testing_a_gui.md, 6.

## Not known

The add-menu painting flipped between a highlighted first row and a plain one (the hover
the press that opened the menu left under the pointer). The cause was not found: beui
re-tests a resting pointer after every layout, so the row should highlight every time or never,
and it did one or the other in all of the roughly 1,000 runs tried. The test parks the pointer
(`leave`), which hides the flip and does not explain it. The settled-painting check above would
have caught it where it happened.

## Why

- A test author cannot be expected to remember every unsettled thing; the tools that capture
  or wait should be the ones that do not return early, and one that does should fail.
- A flake of one in a few hundred runs never fails on the author's machine and fails a pull
  request's CI about every few dozen, so it has to be found by repeating.

## Rejected

- Moving the pointer out of the window before every painting. 20 of the host tests' paintings
  change, and some are about hover (a splitter hovered, a tooltip); a painting that does not
  mean to show hover says so with `leave` instead.
- Making `pause N` before a painting or a geometry check an error. With the pause waiting on
  the apps it is correct for animations, and a lint cannot tell those from a wait for a
  thread.
- `./scripts/test NAME --repeat N`. `./scripts/test` is the pinned bootstrap, which a change
  here would have to release and pin again, and it runs each test on the build server where
  the load cannot be chosen.
- A scheduled CI job repeating every host test. It would show a flake after it landed; it can
  be added over `stress` when the cost is worth it.
- Frame or iteration counts as a wait, in a test or the harness. They are a timeout by
  another name: the frames of a pause run in a moment while a thread is still slow.

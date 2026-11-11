# Host tests stay flake free by what the tools wait for, and a stress run finds the rest

A host test fails under load when it reads something the machine has not finished with: an
app's answer on a thread of its own, a server's push, what a resting pointer hovers. These
rules make that hard to write and easy to find.

- `drive pause` runs each frame of a stopped clock only once the program has nothing to wait
  for (`Session::quiet`), and replies only then. A pause used to step the clock whatever the
  apps had answered, so what an app reports across its own thread (a child's size) landed
  after the pause under load. A running clock is not held: a program that repaints forever is
  never quiet.
- `drive paint` already waits for the program to be quiet, and every input command settles;
  both fail after 30 seconds saying what kept the program busy.
- A frame asked of an app while another is in flight is asked again once that one is
  answered (`Pacing::need`), instead of being cleared with it.
- `./scripts/buck run //crates/be-tool-local:bin -- stress NAME --repeat N --parallel K`
  runs host tests many times at once and counts the failures by message, so a flake shows
  before it reaches a pull request. It is local and on demand.
- The rules for writing a test live in guides/testing_a_gui.md, 6.

## Why

- A test author cannot be expected to remember every unsettled thing; the tools that capture
  or wait should be the ones that do not return early.
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

# Launching a program for `drive`

## Starting any beui program

beui's run functions (`beui::run`, `run_with`, `run_with_renderers`, `run_on`) read two
variables, so a program needs nothing of its own to be driven:

- `BEUI_HEADLESS=WIDTHxHEIGHT[@SCALE]` runs it on `beui::Headless` (crates/beui-adapter-headless),
  which draws offscreen with wgpu and needs no display server. `BEUI_HEADLESS_SCREEN=WIDTHxHEIGHT`
  is the screen a fullscreen window grows to.
- `BEUI_AUTOMATION=PATH` answers `drive` on the Unix socket PATH. It works in a window too.

`be-drive launch` (crates/be-drive) sets both, waits for the program to settle and writes the
`env` file, which puts `drive` on `PATH` pointed at that program. `headless(...)` in
buck/app/drive.bzl makes a crate's `:dev` target from it.

## Options

Arguments after `--` go to the launcher, which passes on those it does not know:

- `--fresh` deletes the program's data first (programs started with `--data`, which block-app is,
  keep it in `~/.cache/be3/NAME/data` through `XDG_DATA_HOME`).
- `--stop` stops it.
- `--size=WIDTHxHEIGHT[@SCALE]` (1100x720 by default) and `--screen=WIDTHxHEIGHT` (1440x900).
- Anything else, such as block-app's `--desktop` (guides/linux_desktop.md), goes to the program.

A program's output goes to `~/.cache/be3/NAME/app.log`. `BE_DRIVE_DIR` moves the directory,
for two copies of one program at once; `drive --socket=PATH` or `BE_DRIVE_SOCKET` picks the
program a command goes to.

## In tests

`be-drive run` takes the same options and is for tests: it starts the program with fresh data
and its output on its own, runs the commands on its standard input as `drive -` does, then
waits for the program to exit, and fails unless every command succeeded and the program exited
cleanly. block-app's smoke test (buck/app/smoke.sh) is `wait` for its shell's pane, then `close`.

## How it works

`beui_core::app::automation` runs the commands inside the runner every adapter shares
(guides/beui.md), so input lands on the queue a window's own events go to and everything
above it - the document, block-app's plugin host, the Wayland compositor - runs as it does
with a window. What a headless run never exercises is the platform underneath: winit's
translation of the system's keys, its clipboard and file dialogs, the window's swapchain and
real input methods. For those, run the program in a window (guides/running/native.md).
The web build answers the same commands through Playwright (guides/running/web.md).

# Driving a program with `drive`

Every beui program can run headless and be driven from the shell: read what is on screen as
text, click and type by name, wait until it settles, take screenshots and paintings. This is
how to see a change working in the real program.

    ./scripts/buck run //crates/be-host:dev
    source ~/.cache/be3/dev/env
    drive tree

`:dev` exists for be-host (signed in, personal organization open on My files), beui-demo
(`//crates/beui-demo:dev`, env in `~/.cache/be3/beui-demo/env`) and be-launcher
(`//crates/be-tool-launcher:dev`). Each builds the program, starts it with no window and returns
once it has settled; running it again restarts it, which is how to pick up a rebuild. The `env`
file puts `drive` on `PATH`, pointed at that program. `drive help` lists every command.

## How it fits together

drive is a layer, crates/beui-layer-drive (guides/beui/programs.md, Layers), sitting in the
stack every platform's runner carries, so its input lands among the platform's own events and
everything above it (the document, be-host's app host, the Wayland compositor) runs as it does
in a window. `Drive`'s doc comment describes the session and its taps. Natively the run
functions serve it on a Unix socket; be-host's web build serves it through its `automate`
export (guides/running/web.md).

A command goes where it can only mean one thing:

- the session's own: `settle`, `pause`, `clock`, `hurry`, `frames`, `close`;
- reading the program: `tree`, `ids`, `find`, `paint`, `overflows`, `windows`, `actions`, ...;
- the platform's side, below every layer: `shot`, the file and save dialogs drive stands in
  for, the clipboard, resizing the headless window, focus, files dragged in;
- input (pointer, touch, pen, keys) and `state`, which reach the program.

A headless run never exercises the platform underneath: winit's key translation, the system
clipboard and file dialogs, the swapchain, real input methods. For those, run in a window
(guides/running/native.md).

## Starting any beui program

beui's native run functions read these variables (Android reads none), so any program can be
driven without code of its own:

- `BEUI_PLATFORM`: `winit` (default), `headless`, or one the program lists (be-host's `drm`).
- `BEUI_LAYERS`: layers between the platform and the program, nearest the platform first:
  `space`, `xr`, `drive`. Each `drive` is a tap of one session under the layer above it.
- `BEUI_HEADLESS_WINDOW=WxH[@SCALE]` (1100x720) and `BEUI_HEADLESS_SCREEN=WxH`, headless only.
- `BEUI_RENDERER=none`: headless with no GPU. Frames are laid out and painted but not drawn, so
  `drive paint` works and `drive shot` does not; be-host's apps then draw nothing on the GPU
  either.
- `BEUI_DRIVE_SOCKET=PATH`: the socket drive answers on, required with `drive` in
  `BEUI_LAYERS`. It works in a window too.

`be-drive launch` (crates/be-tool-drive), which every `:dev` runs (`headless(...)` in
buck/program/drive.bzl), sets all of these, waits for the program to settle and writes the `env`
file. Arguments after `--` go to it:

- `--fresh` deletes the program's data first; `--stop` stops it.
- `--size=WxH[@SCALE]`, `--screen=WxH`.
- `--latency=MS` adds MS to every round trip to the server (`:dev`'s is in-process and
  otherwise instant). `settle` does not wait for the server, so wait for what should appear.
- `--pin-clock` starts the wall clock at the moment tests use (2026-03-02 09:41 UTC) in UTC,
  moving with the program's clock. Without it `:dev` runs on real time.
- Anything else goes to the program, such as be-host's `--desktop` (guides/desktop.md).

Output goes to `~/.cache/be3/NAME/app.log`. `BE_DRIVE_DIR` moves that directory, for two copies
at once; keep it short (`/tmp/be3-123`), since the socket inside it must fit a Unix socket path.
`drive --socket=PATH` or `BE_DRIVE_SOCKET` picks the program a command goes to.

be-host shows a crash report from its last run (`last-panic.txt` in its data) on its error
screen at the next start, which is where `:dev` then settles; `drive tree` reads it.

A program that hosts programs (be-host `--desktop`, whose compositor is be-host-wayland) gives
drive the environment a client needs: `drive exec PROGRAM [ARGS]` runs one inside it, and
`eval "$(drive env)"` sets that environment in the shell.

For tests, `be-drive run` (and `be_tool_drive::Driven` from Rust) starts the program with fresh
data and a pinned clock, runs the commands on standard input, and fails unless every command
succeeds and the program exits cleanly. crates/be-host-tests uses it (guides/testing_a_gui.md, 6).

## Reading the screen

`drive tree` is the accessibility tree, one node per line, indented under its parent:

    Window "Block" at 0,0 size 1100x720
      Dialog "Invite member" at 360,180 size 380x300
        TextInput "Email address" focused at 380,260 size 340x36
        Button "Send invitation" at 380,420 size 116x36 #invite.send
      Pane "Checklist" at 253,51 size 837x608
        CheckBox "buy milk" toggled=True at 307,307 size 624x18

Coordinates are pixels from the window's corner. In be-host each app region is a `Pane` (guides/apps/editor.md, Accessibility) named
after its app, holding the nodes its editor describes, so apps read like the host's own
controls. An app's anchored view (a menu or popover past its frame) is an untitled container
beside its `Pane`.

- `ids` lists every test id and where it is, apps included. Test ids are the steadiest names.
- `find`, `reveal` (scrolls into view first) and `shown` say where a target is.
- `hit TARGET` says what a press there would reach, innermost first, and what it would dismiss:
  use it when a press goes to the wrong place. It sees the host's document only.
- `comps [--internals]` is the host document's component tree, as the inspector's Comp tab.
- `state` is what the tree does not say: window size and focus, the cursor, held keys and
  pointers, IME composition, open dialogs, and `settled` or `busy:` with the reasons.
- `frames` gives the frame count and longest wall time since it was last asked, with each app's
  steps on a line of its own; it is wall time, for benchmarks (guides/performance.md).
- `actions [all]` lists the command palette's actions where focus is (guides/beui_keyboard.md).
- `shot FILE.png [TARGET]` is a PNG of what the platform shows (under space, the 3D view).
- `paint FILE.paint [TARGET] [in WINDOW]` is a paint snapshot (guides/testing_a_gui.md, 4) with
  every app's painting where the host composites it, which compares exactly where a PNG would
  not; `paint FILE.png` draws it flat. What an app draws through its own `Drawing`, a web view
  or a Wayland window is a placeholder.
- `overflows` lists what the window shows out of bounds, host and apps included.

## Targets

- `#TEST_ID`, or `#TEST_ID+X,Y` for a point inside that node;
- `X,Y` in the tree's pixels, or `X,Y,W,H` for an area;
- text in one line of the tree: `'"Send invitation"'`, `'Button "Create"'`. A match inside
  another match is the same target; unrelated matches are an error that lists them.

Targets are found in the main window; `in WINDOW` at the end of an input command picks another
from `drive windows` (each display's window in a login session, say).

A command that presses, points or types at a target first scrolls it into view when a scroll
hides or cuts it (the accessibility `ScrollIntoView`), then aims at the middle of the part that
shows. `shot` and `paint` reveal and crop the same way. `BeuiTest::click` does the same.

## Acting

    drive click TARGET [right|middle|double]
    drive down TARGET; drive pause 600; drive move TARGET; drive up
    drive drag FROM TO [STEPS] [BUTTON]
    drive wheel TARGET 3            drive scroll TARGET 300       drive zoom TARGET 2
    drive tap TARGET                drive pinch TARGET 2
    drive touch down 0=A 1=B; drive touch move 0=C 1=D; drive touch up
    drive pen draw FROM TO [STEPS [PER_FRAME]]
    drive type 'buy milk'           drive key ctrl+z Enter shift+Tab
    drive ime compose ni; drive ime commit 你
    drive act ACTION_ID [PANE]      drive a11y TARGET [ACTION [VALUE]]

`pen draw` packs PER_FRAME samples into each frame, as a fast stylus or a backed-up program
delivers them. `motion DX DY` is relative motion for a locked pointer. `a11y` acts through the
accessibility tree, the path a screen reader takes. `drive -` reads commands from standard
input, one a line, and stops at the first that fails:

    drive - <<'EOF'
    click "Add a root block"
    wait 'Pane "Checklist"'
    shot checklist.png 'Pane "Checklist"'
    EOF

## Waiting

Every input command waits until the program settles, then prints the tree's changes as `-` and
`+` lines (`--changes=N|all|0`). `--no-settle` answers after the frame that took the input, to
see the program part way through something. A command gives up after 30 s
(`--timeout=SECONDS`) and names what kept the program busy.

A program has settled once a frame asks for no other and `Program::busy` gives no reason.
be-host is busy while an app starts, owes a frame or has unanswered input, while an editor on
screen waits for content, while an account request is pending, and while the block store has
work queued or edits to seal or send. It is never busy on its error screen.

Work the program cannot see is not waited for:

- A server push: use `drive wait TEXT` or `drive gone TEXT`, which return once a line contains
  TEXT or none does. `wait TEXT over TARGET` also asks that the node cover a point, for
  something that grows, such as a stroke reaching the pen.
- A hosted Wayland or X11 program redrawing after input (guides/desktop/wayland.md, Frame
  pacing): the tree does not show what it draws, so `drive pause 200` before a `shot`.

### The clock

`drive clock stop` stops the program's clock. Time then passes only with `drive pause MS`, one
frame at a time, so an animation can be captured at any point the same way every run and a
program that animates forever still settles. A pause runs its frames at once, costing only
the frames, not the time it names. `drive clock run` restarts it. With the clock stopped:

- a menu or sheet that closes stays in the tree until its animation ends, so `gone` after
  closing it needs a `pause` first;
- anything else timed (an app's frame timeout, a type module grouping edits into an undo step)
  waits for `pause` too.

A driven program takes shortcuts a person never sees so that it settles sooner: be-host seals
edits at once instead of after the usual pause (`Context::hurried`). `drive hurry off` turns
them off, to time what a person waits for:
`drive hurry off; drive --no-settle click TARGET; time drive wait TEXT`.

## The headless window

- `drive resize 390x844@3` resizes it and sets its scale (a phone's layout);
  `drive resize screen 1920x1080` sets the screen a fullscreen window fills.
- `focus` and `blur`; `blur` cancels every pointer and releases every held key, as winit does.
- `clipboard [TEXT]` reads or sets the window's clipboard.
- File and save dialogs wait for drive: `upload FILE TARGET`, `save FILE TARGET`, `dismiss`.
- `grab FILE...`, then `move`, `drop [TARGET]` or `ungrab`, drags files in from outside.
- `close` asks the window to close, as its close button does.

## Space and headsets

With other layers in the stack, drive has a tap under each. Input still reaches the program by
default; `--into=LAYER` sends it into that layer from below, as the platform's input would
come: `drive --into=space click '"Save"'` clicks through space's projection wherever the panel
is. Under `--into`, `X,Y` is that place's own pixels. `BEUI_LAYERS=space` on `:dev` runs
`drive,space,drive` (guides/beui/space.md).

- `drive shot` takes the 3D view; `drive paint` still paints the program's own window flat.
- Space's steering takes keys and motion from below: `drive --into=space keydown ctrl shift w`
  walks. A held walking key keeps frames coming, so the command waits out its timeout and walks
  far; to move a set amount, stop the clock, hold with `--no-settle`, `pause`, and release.
- Holding Escape for 700 ms lets the pointer go; the next click into space only grabs it again.
- `BEUI_LAYERS=xr` puts space on an OpenXR headset (guides/beui/space.md, Headsets);
  `drive paint in platform` paints the 3D scene below space.
- `drive xr left|right ...` works the controllers' lasers (guides/beui/space.md, Lasers):
  `point TARGET`, `trigger 1|0`, `click TARGET`, `squeeze 1|0` to carry a panel,
  `pose X Y Z [YAW PITCH]` in metres (eye height 1.6), `lost`. A laser turned by less than
  0.15 rad is smoothed over frames, which with the clock stopped needs a `pause`.

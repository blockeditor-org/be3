# Driving an app with `drive`

Every beui program can run headless and be driven from the shell: read what is on screen as
text, click and type by name, wait until it settles, and take screenshots. This is the way to
see a change working in the real app. Start it with its `:dev` target:

    ./scripts/buck run //crates/block-app:dev
    source ~/.cache/be3/dev/env
    drive app tree

`:dev` exists for block-app (signed in, with the personal organization open on My files), beui-demo
(`//crates/beui-demo:dev`, `~/.cache/be3/beui-demo/env`) and be-launcher
(`//crates/be-launcher:dev`). Each builds the program, starts it with no window and returns
once it has settled; running it again restarts it, which is how to pick up a rebuild. The
`env` file puts `drive` on `PATH`, pointed at that program; `drive help` lists the commands.

## Taps and the session

drive is a layer (crates/beui-layer-drive, guides/beui.md "Layers") with two taps that share
one socket, one command at a time and one clock:

- the **screen** tap, nearest the platform: input in the platform window's pixels, the pixels
  the platform shows (`shot`), and the services drive stands in for (file and save dialogs,
  the clipboard, resizing the headless window, focus, files dragged in);
- the **app** tap, nearest the app: the app's accessibility tree, targets named in it, input
  that reaches the app without passing the layers between the taps, the app's paintings per
  window and its overflows.

Every command names its tap, `drive screen COMMAND` or `drive app COMMAND`; options such as
`--no-settle` go before the tap. The session's own commands name none: `settle`, `pause`,
`clock`, `hurry`, `frames`, `close` and `help`. `drive -` (commands from standard input) and `drive exec` are the
`drive` client's own. Input commands (pointer, touch, pen, keys,
`state`) go to either tap and enter the stack there. A command sent to a tap that cannot serve
it is an error that names the tap that can; nothing is rerouted. With one drive layer in the
stack (`BEUI_LAYERS=drive`, what `:dev` gives a program with no other layer), that layer is both
taps, so either name reaches every command and the screen tap finds names as the app tap does.
With a layer between them (`BEUI_LAYERS=drive,space,drive`) the screen tap is below it and the
app tap above it, and the screen tap takes only `X,Y` and `X,Y,WIDTH,HEIGHT` targets.

## Starting any beui program

beui's run functions (`beui::run`, `run_with`, `run_with_renderers`, `run_on`) read these
variables (on every platform but Android, which reads none), so a program needs nothing of its
own to be driven:

- `BEUI_PLATFORM` picks the platform: the platform's own (`winit`, or the one the program hands
  `run_on`, such as block-app's `drm`) by default, or `headless`. A
  name it does not know is an error that lists the ones it takes
  (`beui_core::runner::environment::choose`).
- `BEUI_LAYERS` stacks layers between the platform and the app (guides/beui.md, "Layers"):
  names separated by commas, the one nearest the platform first: `space` and `drive`; an
  unknown name is an error. `drive` once is one drive layer; `drive` twice is one session's
  screen tap (the first) and app tap (the second), and more is an error.
- `BEUI_PLATFORM=headless` runs it on `beui::Headless` (crates/beui-platform-headless), which draws
  offscreen with wgpu and needs no display server. `BEUI_HEADLESS_WINDOW=WIDTHxHEIGHT[@SCALE]`
  (1100x720 by default) is its window and `BEUI_HEADLESS_SCREEN=WIDTHxHEIGHT` the screen a
  fullscreen window grows to; either is an error under another platform.
- `BEUI_RENDERER` picks the renderer: `wgpu` (the default) or `none`, which runs headless with no
  GPU at all (`beui_core::renderer::NoRenderer`): every frame is laid out and painted as usual,
  but nothing draws the painting, so `drive app paint` works and `drive screen shot` does not. block-app
  then gives its plugins surfaces it does not render, so they run their frames without a GPU
  either. `none` needs the headless platform.
- `BEUI_DRIVE_SOCKET=PATH` is the Unix socket the drive layer answers on; `BEUI_LAYERS` naming
  `drive` needs it, and it is an error without `drive` there. It works in a window too:
  `BEUI_LAYERS=drive BEUI_DRIVE_SOCKET=/tmp/app.sock`.
- `BEUI_LAYERS=drive,space,drive` shows the app as a panel in the VR simulator (guides/beui.md,
  "Space: the VR simulator layer"), on whichever platform `BEUI_PLATFORM` picks, with the screen
  tap below space and the app tap above it: `drive screen shot` takes the 3D view, `drive app
  click '"Save"'` clicks the app's button wherever its panel is, and `drive screen click X,Y`
  puts space's cursor at that point of the platform window, as the mouse does, so the click
  goes to whatever panel or chrome is in front of it. The app tap's targets are in the app's
  main window unless a command names another with `in WINDOW` (the other panels); the panels'
  own placement is tested through `Space` and a `Runner` (`InSpace` in crates/beui/src/tests.rs). Space's steering takes keys and
  motion from the screen tap: holding a walking key keeps frames coming, so the command waits
  out its settle timeout and the head walks far; turn with `drive screen motion` while Ctrl
  and Shift are held instead. To walk or roll (Q/E) by a set amount, stop the clock and time
  the hold, with Ctrl and Shift held: `drive clock stop; drive --no-settle screen keydown q;
  drive pause 500; drive --no-settle screen keyup q; drive clock run`. Holding Escape lets the
  pointer go: `drive screen keydown escape; drive pause 700; drive screen keyup escape` (the
  app sees the press and the release). After that, or a lost focus, the next screen click only
  grabs the pointer again and reaches nothing.
  `BEUI_LAYERS=drive,xr,drive` shows space on an OpenXR headset instead (guides/beui.md,
  "Headsets"), waiting for one, with a notice over the simulator, while there is none; the
  window is the headset's mirror. `drive screen paint` paints what reaches the platform, the 3D scene below space,
  as `drive screen shot` shows it.
  `drive screen xr HAND ...` (HAND is `left` or `right`) works the controllers' lasers, which act
  as fingers (guides/beui.md, "Lasers"): they enter below space, as a headset's do. `xr right
  point TARGET` aims that hand's laser from where the hand is at TARGET, found in the app's tree,
  `xr right trigger 1` and `xr right trigger 0` press and let go there (`xr right click TARGET`
  does all three), `xr right squeeze 1` carries the panel the laser is on until `xr right squeeze
  0`, `xr left pose X Y Z [YAW PITCH]` puts a hand at a point in metres (eye height is 1.6)
  turned YAW degrees left and PITCH up from -Z, which is how to move a carried panel or spread
  both grips, and `xr right lost` stops tracking it. Two hands pressed on one panel and pointed
  apart pinch it. A laser turned by less than 0.15 rad is smoothed and settles over frames, which
  with the clock stopped needs `drive pause`.

`be-drive launch` (crates/be-drive) sets the headless platform, its sizes and the socket, and
adds drive to the `BEUI_LAYERS` it finds: `drive` alone when it names none, the taps around
whatever it names when it names no drive (so `BEUI_LAYERS=space ./scripts/buck run
//crates/block-app:dev` runs `drive,space,drive`, the VR simulator), and the list as it is
when it names drive itself. It waits
for the program to settle and writes the `env` file. It listens on the drive socket itself
and hands the program the listener (`BEUI_DRIVE_LISTENER`, its file descriptor), so it can
ask the program to settle at once rather than watch for the socket to appear. `headless(...)` in buck/app/drive.bzl makes a crate's `:dev` target from it.
Arguments after `--` go to the launcher, which passes on those it does not know:

- `--fresh` deletes the program's data first (programs started with `--data`, which block-app is,
  keep it in `~/.cache/be3/NAME/data` through `XDG_DATA_HOME`).
- `--stop` stops it.
- `--size=WIDTHxHEIGHT[@SCALE]` (1100x720 by default) and `--screen=WIDTHxHEIGHT` (1440x900).
- `--latency=MS` adds MS milliseconds to every round trip between the program and its server,
  half each way, to see what a distant server does to a change; `:dev`'s server is in-process
  and otherwise answers at once. It sets `BE_LATENCY`, which be-client's native transport
  reads, so a program started any other way can set it too. `settle` waits only until nothing
  is left to draw, not for the server, so wait for what should appear with `wait`, or poll
  `tree` after an input sent with `--no-settle`.
- Anything else, such as block-app's `--desktop` (guides/linux_desktop.md), goes to the program.

A program's output goes to `~/.cache/be3/NAME/app.log`. `BE_DRIVE_DIR` moves the directory,
for two copies of one program at once, so agents sharing a machine each give theirs a
directory of its own. Keep it short, such as `/tmp/be3-123`: the drive socket (`drive.sock`) is in it,
and a Unix socket's path fits in 107 bytes. `drive --socket=PATH` or `BE_DRIVE_SOCKET` picks
the program a command goes to.

block-app keeps a crash report from its last run in its data (`last-panic.txt`) and shows it
on its error screen at the next start, which is where `:dev` then settles; `drive app tree` reads
it, and the next `:dev` starts normally, since the report is shown once.

An app that hosts programs (block-app under `--desktop`, whose compositor is be-wayland)
tells drive the variables a program needs to open its windows inside it (`App::environment`):
`drive exec PROGRAM [ARGUMENTS]` runs one with them, in the foreground with its output on the
terminal (end the line with `&` to go on driving), and `eval "$(drive app env)"` sets them in the
shell.

`be-drive run` takes the same options and is for tests: it starts the program with fresh data
and its output on its own, runs the commands on its standard input as `drive -` does, then
waits for the program to exit, and fails unless every command succeeded and the program exited
cleanly. From Rust, `be_drive::Driven` does the same: it starts a program with fresh data in a
directory of its own, sends it commands and closes it, which is how crates/app-tests drives
block-app (guides/testing_a_gui.md, 6).

## Reading the screen

`drive app tree` is the accessibility tree, one node per line, indented under its parent:

    Window "Block" at 0,0 size 1100x720
      Dialog "Invite member" at 360,180 size 380x300
        TextInput "Email address" focused at 380,260 size 340x36
        Button "Send invitation" at 380,420 size 116x36
          Label value="Send invitation" at 392,430 size 92x15
      Pane "Checklist" at 253,51 size 837x608
        CheckBox "buy milk" toggled=True at 307,307 size 624x18
        Slider "Load" value=0.8 at 291,476 size 771x20

Coordinates are pixels from the window's corner. `offscreen` marks a node scrolled or clipped
out of view, `focused` the one with keyboard focus and `selected` a selected row or option. In block-app each plugin region is a
`Pane` named after its plugin, holding the nodes its editor describes (guides/adding_a_plugin_editor.md),
so dialogs, the file tree and every editor read like the host's own controls.

- `drive app ids` lists every test id in the tree with where it is, the plugins' included. Test ids
  (guides/testing_a_gui.md) are the steadiest way to name a control; `drive app tree` shows them
  too, as `#id` after a node's state, and a node with nothing else to show reads as a
  `GenericContainer`.
- `drive app find TARGET` says where a target is.
- `drive app hit TARGET` says what a press there would reach without pressing: the `Interactive`s
  along the hit path, innermost first, each with its kind, rect, test ids, the components it was
  made in and what it handles, then what forwarded input goes to and which overlays the press
  would dismiss. Use it when a press goes to the wrong place or nowhere. It sees the host's own
  document only; in block-app a press over a plugin ends at that plugin region's catcher.
- `drive app comps` prints the host document's component tree with module-qualified names, text
  leaves and `#test_id`s. Caller-supplied children appear beneath the component; components
  built by its body are hidden under `internals`. `drive app comps --internals` includes those
  implementation trees too. This is the same tree as the inspector's Comp tab; the inspector
  initially collapses internals of components defined in `beui_*` crates and expands others.
  Like `hit`, this sees the host document, not plugin documents.
- `drive app state` is what the tree does not say: the window's size, scale and focus, whether it is
  fullscreen, the cursor's shape, the pointer and the fingers down, held keys, an input
  method's composition or where the app asks for one and whether it shows the on-screen
  keyboard or holds it back, a back gesture under way, files being dragged in, and any open file
  dialog.
- `drive frames` says how many frames ran since the last `drive frames` (or the start) and the
  longest one's wall time: the window's own, from taking its input to painting, and on a line
  each what the app ran elsewhere and reports through `App::other_frames`, which in block-app is
  each plugin's steps by its id (a step takes the plugin's input, makes its edits and draws its
  frame). A block-app frame waits at most 8 ms for its plugins, so a slower plugin step shows
  only on the plugin's line. It is wall time, not the app's clock, so it is for benchmarks
  (guides/performance.md), not tests that compare.
- `drive app actions` lists the actions the command palette would offer where the focus is
  (guides/beui_keyboard.md), with their shortcuts, and each plugin's under its `Pane`;
  `drive app actions all` lists every registered one.
- `drive screen shot FILE.png [TARGET]` writes a PNG of what the platform shows (under space, the 3D view), or of one target, where that target is in the frame it captures.
- `drive app windows` lists the app's windows, each one's number and size in pixels.
- An input command (`click`, `move`, `down`, `up`, `drag`, `wheel`, `scroll`, `tap`, ...) may end
  with `in WINDOW`, a number `windows` lists: its targets are then found among that window's
  nodes only, and its input goes to that window, in its points. That is how to reach a window
  other than the main one, such as one display's window under `--session`, where every display
  shows its own lock screen and bar with the same test ids:
  `drive app click '#lock.password' in 3`.
- `drive app paint FILE.paint [TARGET] [in WINDOW]` writes what the app's main window (or the
  window numbered WINDOW) paints as a paint snapshot
  (guides/testing_a_gui.md, 4), every plugin's painting included where the host composites it,
  so it compares exactly where a PNG would not; `./scripts/buck run //crates/local-tools:bin -- rasterize SNAPSHOT.paint OUTPUT.png [FRAME|all]` turns it into a PNG and
  `example-dump` lists its shapes. It waits until the app settles and crops to where TARGET is
  in the frame it paints, then asks every running
  plugin for what it last painted, through the plugin's `paintings` export (paint-format's
  `ScreenPaintings`). What a plugin draws through a
  `Drawing` of its own, a web view or a Wayland window is a placeholder rectangle, as in any
  snapshot.
- `drive app overflows` lists, one per line, what the window shows out of bounds
  (beui-core's `Document::overflows`, guides/testing_a_gui.md, 4), the host's documents and every
  plugin shown included. Each document checks itself on every frame while it is driven, and a
  plugin's findings come back with its painting, so it waits as `paint` does.

## Targets

A command that acts on something takes a TARGET:
- `#TEST_ID`, such as `#checklist.add`;
- `#TEST_ID+X,Y`, the point X,Y pixels right of and below that node's top left corner, for a
  gesture that must land somewhere particular inside it, such as `#wheel+60,60`;
- `X,Y` in the tree's pixels, or `X,Y,WIDTH,HEIGHT` for an area (to `shot`); where a layer sits
  between the taps, these are the only targets the screen tap takes, in the platform window's
  pixels;
- text found in one line of the tree, such as `'"Send invitation"'` or `'Button "Create"'`.
  A match inside another match counts as that one, so a button and its own label are one
  target; when several unrelated lines match, the error lists them.

## Acting

`drive help` lists every command; these are the kinds. Each goes to either tap; the app tap is
the one that finds names, so the examples use it.

    drive app click TARGET [right|middle|double]
    drive app move TARGET                         hover
    drive app down TARGET; drive pause 600; drive app move TARGET; drive app up   any gesture, step by step
    drive app drag FROM TO [STEPS]
    drive app wheel TARGET 3                      three mouse-wheel ticks down; a second number turns sideways
    drive app scroll TARGET 300                   a touchpad scroll of 300 pixels
    drive app zoom TARGET 2                       a touchpad pinch
    drive app leave                               the pointer leaves the window
    drive app motion 200 0                        a mouse moving 200 pixels right with the pointer locked
    drive app tap TARGET                          touch, finger 0
    drive app swipe FROM TO
    drive app pinch TARGET 2                      two fingers spreading apart
    drive app touch down 0=TARGET 1=TARGET; drive app touch move 0=... 1=...; drive app touch up
    drive screen xr right click TARGET           a controller's laser, in space: point, trigger,
                                                  squeeze, pose and lost too (see above)
    drive app pen draw FROM TO [STEPS [PER_FRAME]]   a stylus stroke of STEPS samples (8), its pressure
                                              rising and falling, PER_FRAME (1) in each frame
    drive app pen move TARGET; drive app pen down TARGET 0.8; drive app pen move TARGET 0.3; drive app pen up
    drive app pen move TARGET 0.5 400 4           there in 400 samples, 4 in each frame, as a stylus
                                              sends them; down, this draws part of a stroke
    drive app pen leave                           the pen goes out of range
    drive app back                                the system back gesture; back 0.4 holds it part way,
                                              back commit and back cancel finish it
    drive app type 'buy milk'                     keys into what has focus; a newline presses Enter
    drive app key ctrl+z Enter shift+Tab          chords, each pressed and let go in turn
    drive app keydown alt; drive app key tab tab; drive app keyup alt
    drive app ime compose ni; drive app ime commit 你  an input method's composition
    drive app act ACTION_ID [PANE]                run an action the way the command palette does
    drive app a11y TARGET [ACTION [VALUE]]        what a screen reader does: click, focus, increment,
                                              set VALUE, expand, scroll-down, ...

`pen draw` presses in a frame of its own and lifts in another; PER_FRAME packs that many of
the samples between into each frame, as a fast stylus or a backed-up app delivers them.
`touch` names each finger (`0=TARGET`, `1=TARGET`) so several move in one frame, which is how
to make a pinch of your own; `touch up` with no finger lifts them all, and `touch cancel` takes
them back the way the system does when it claims a gesture. `keydown` on a key that
is already down repeats it. `motion DX DY` is relative motion (`Input::PointerMotion`), what a
locked pointer reports: it reaches what holds pointer lock, such as a 3D view's look, and turns
the head in space while Ctrl and Shift are held (`drive screen keydown ctrl; drive screen keydown
shift; drive screen motion 200 0; drive screen keyup shift; drive screen keyup ctrl`, through the
screen tap below space), and otherwise moves space's cursor. `a11y TARGET` with no action prints the node a screen reader would
act on and the actions it offers; acting goes through the accessibility tree rather than the
pointer or keys, the path a screen reader takes, and reaches plugins' nodes too.

A pointer gesture that starts somewhere the pointer is not (`click`, `down`, `drag`) first moves
it there in a frame of its own, as a person's pointer arrives and hovers before it presses: what
shows only on hover, such as a scrollbar, is there to be pressed.

Every command that gives input waits until the app has settled and prints what changed in
the tree, `-` and `+` lines, so there is no need to wait or read the whole tree again. A long
change is cut to its first 60 lines; `drive --changes=N app COMMAND` (or `all`) prints more, and
`--changes=0` nothing, without comparing the trees, which a benchmark wants. The app
has settled once a frame asks for no other and `App::busy` is false; block-app is busy while a
plugin is starting, owes a frame, holds unanswered input or has yet to answer a `paint`, while
an editor on screen waits for its block's content, while a plugin's change arrived after the
frame's layout and is shown only by the next, while an account or organization request is
pending, and while the block store has commands to carry out, edits it owes a seal (which a
driven app asks for at once unless `hurry` is off, below) or is
still loading the organization's graph, and never while it shows its error screen. `drive --no-settle app COMMAND` answers after the frame that took the input
instead, to see the app part way through something, such as a plugin still loading. Work the
app cannot see, such as a server push, is waited for with `drive app wait TEXT` or `drive app gone
TEXT` (or `#TEST_ID`), which return once a line of the tree contains TEXT or none does;
`drive app wait TEXT over X,Y` also asks that the line's node cover that point, for something that
moves or grows, such as a stroke reaching the pen;
`drive settle` waits on its own, and `drive pause MILLISECONDS` lets time pass by the app's
own clock. Each command gives up after 30 seconds, or `drive --timeout=SECONDS`.

`drive clock stop` stops the app's clock: time then passes only with `drive pause`, a frame at
a time, so an animation can be stopped at any point and a screenshot of it taken the same way
every run, and an app that animates forever still settles. `drive clock run` starts it again.
Anything else waiting on time, such as a plugin's frame timeout or a type module's clock (which
groups quick edits into one undo step), waits for `pause` too, and a frame waits for the plugins it asked to draw however long they take, so `--no-settle` answers with the same frame on a loaded machine as on an idle one.

A driven app may take shortcuts a person never sees so that it settles sooner: block-app seals
edits at once rather than after the pause a person's edits wait for. An app asks
`Context::hurried` before taking one. `drive hurry off` turns them off until `drive hurry on`,
so the app waits as it does in a window; time how long something takes to show with it off,
such as `drive hurry off; drive --no-settle app click TARGET; time drive app wait TEXT`.

`drive -` reads commands from standard input, one a line, quoted as a shell would, prints each
before its answer and stops at the first that fails:

    drive - <<'EOF'
    app click "Add a root block"
    app click 'Label value="Checklist"'
    app wait 'Pane "Checklist"'
    screen shot checklist.png 'Pane "Checklist"'
    EOF

`type` presses the key that makes each character, with Shift where a US layout needs it,
with each key's evdev code, so a Wayland program's window takes it as it takes `key` and
`keydown`. A character no key makes, such as `é`, is sent as text, which only beui takes.

## The headless window

A headless program's window is simulated, and drive can do to it what a person does to a
real one:

- `drive screen resize 390x844@3` resizes it and sets its scale, which is how to see a phone's
  layout; `drive screen resize screen 1920x1080` sets the screen it fills when the app asks for
  fullscreen. Only a headless window can be resized this way.
- `drive screen focus` and `drive screen blur` give it focus or take it away. `blur` does what a
  window blur does on winit and the web: it cancels every pointer (buttons, fingers and
  the pen) and lets go of every key drive holds.
- `drive screen clipboard` prints the window's clipboard and `drive screen clipboard TEXT` sets it; the
  app's own copy and paste use it, and `drive app key ctrl+v` pastes it. In a headless window
  the clipboard is the window's own; in a real one it is the system's.
- A file dialog the app opens waits for drive, in any window while drive is attached:
  `drive screen upload FILE TARGET` clicks TARGET and answers the dialog it opens with FILE
  (without TARGET it answers one already open), and `drive screen dismiss` cancels it. A save
  dialog waits the same way: `drive screen save FILE TARGET` clicks TARGET and writes what the
  dialog saves to FILE, and the app hears it was saved. `drive screen dismiss` cancels the oldest
  dialog of either kind, and `drive screen state` lists the ones open.
- `drive screen grab FILE...` drags files in from outside, over the pointer; `drive app move` carries
  them, to try what hovering with them shows, `drive screen drop [TARGET]` drops them and
  `drive screen ungrab` takes them away again.
- `drive close` asks the window to close, as its close button does, in a window too. The app
  then exits, or stays open, to ask about unsaved changes, and answers with what changed.

## How it works

`beui_layer_drive::Drive` is a `Layer` (guides/beui.md, "Layers") in the stack every platform's
runner carries, so its input lands among the platform's own events, and everything above it -
the document, block-app's plugin host, the Wayland compositor - runs as it does with a window.
`Drive::new(inbox)` is one layer serving both taps; `Drive::taps(inbox)` is a session's screen
tap and app tap, two layers sharing it (`Rc<RefCell<Session>>`). The session takes each
command, starts it, steps its clock (in `Layer::now`) and answers it from the screen tap's
hooks, the lowest drive layer; the app tap's `input` puts the app tap's input in just below the
app. The screen tap stands in for the file and save dialogs (`pick_file`, `save_file`) and
reads the clipboard and resizes through the services below it; the native run functions serve
its `Inbox` on `BEUI_DRIVE_SOCKET` (`beui_layer_drive::socket`), and block-app's web build hands
it one its `automate` export fills. A driven app keeps the accessibility tree a screen reader would get, as the app
does in a window, but writes it out as text only when a command reads it (`tree`, a target, a
`wait`, the changes it answers with), and then describes again only the nodes that changed
or moved (`AccessibilityDump`), so what a frame costs is what it costs in a window. What a headless run never exercises is the platform underneath: winit's
translation of the system's keys, its clipboard and file dialogs, the window's swapchain and
real input methods. For those, run the program in a window (guides/running/native.md).
The web build answers the same commands through Playwright (guides/running/web.md).

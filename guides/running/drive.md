# Driving an app with `drive`

Read this first to run any app. Every beui program can run headless and be driven from the
shell: read what is on screen as text, click and type by name, wait until it settles, and take
screenshots. This is how to see a change working in the real app. For what a headless run does
not exercise, see `guides/running/native.md` (a real window), `guides/running/web.md` and
`guides/running/android.md`.

## Starting it

```
./scripts/buck run //crates/block-app:dev
source ~/.cache/be3/dev/env
drive tree
```

| Program | Target | `env` file |
|---|---|---|
| block-app, signed in with a workspace called Dev open | `//crates/block-app:dev` | `~/.cache/be3/dev/env` |
| beui-demo | `//crates/beui-demo:dev` | `~/.cache/be3/beui-demo/env` |
| be-launcher | `//crates/be-launcher:dev` | `~/.cache/be3/be-launcher/env` |

- Each `:dev` builds the program, starts it with no window and returns once it has settled.
- Running it again restarts it, which is how to pick up a rebuild.
- The `env` file puts `drive` on `PATH`, pointed at that program. `drive help` lists the
  commands.
- The program's output goes to `~/.cache/be3/NAME/app.log`, where NAME is `dev`,
  `beui-demo` or `be-launcher`.

### Options

Arguments after `--` go to the launcher, which passes on those it does not know:

| Option | Effect |
|---|---|
| `--fresh` | deletes the program's data first (programs started with `--data`, as block-app is, keep it in `~/.cache/be3/NAME/data` through `XDG_DATA_HOME`) |
| `--stop` | stops it |
| `--size=WIDTHxHEIGHT[@SCALE]` | the window (1100x720 by default) |
| `--screen=WIDTHxHEIGHT` | the screen a fullscreen window grows to (1440x900 by default) |
| anything else | goes to the program, such as block-app's `--desktop` (`guides/linux_desktop.md`; other flags in `guides/running/native.md`) |

- `BE_DRIVE_DIR` moves the `~/.cache/be3/NAME` directory, to run two copies of one program at
  once.
- `drive --socket=PATH` or `BE_DRIVE_SOCKET` picks the program a command goes to.

## Reading the screen

`drive tree` prints the accessibility tree, one node per line, indented under its parent:

```
Window "Block" at 0,0 size 1100x720
  Dialog "Invite member" at 360,180 size 380x300
    TextInput "Email address" focused at 380,260 size 340x36
    Button "Send invitation" at 380,420 size 116x36
      Label value="Send invitation" at 392,430 size 92x15
  Pane "Checklist" at 253,51 size 837x608
    CheckBox "buy milk" toggled=True at 307,307 size 624x18
    Slider "Load" value=0.8 at 291,476 size 771x20
```

- Coordinates are pixels from the window's corner.
- `offscreen` marks a node scrolled or clipped out of view; `focused` the one with keyboard
  focus.
- Test ids show as `#id` after a node's state. A node with nothing else to show reads as a
  `GenericContainer`.
- In block-app each plugin region is a `Pane` named after its plugin, holding the nodes its
  editor describes, so dialogs, the file tree and every editor read like the host's own
  controls.

| Command | What it prints |
|---|---|
| `drive tree` | the accessibility tree |
| `drive ids` | every test id in the tree with where it is, the plugins' included; test ids (`guides/testing.md`, Test ids) are the steadiest way to name a control |
| `drive find TARGET` | where a target is |
| `drive hit TARGET` | what a press there would reach, without pressing (below) |
| `drive state` | what the tree does not say (below) |
| `drive actions` | the actions the command palette would offer where the focus is (`guides/beui/input.md`), with their shortcuts, and each plugin's under its `Pane`; `drive actions all` lists every registered one |
| `drive shot FILE [TARGET]` | writes a PNG of the window, or of one target |

`drive hit TARGET` lists the `Interactive`s along the hit path, innermost first, each with its
kind, rect, test ids, the components it was made in and what it handles, then what forwarded
input goes to and which overlays the press would dismiss. Use it when a press goes to the wrong
place or nowhere. It sees the host's own document only: in block-app a press over a plugin ends
at that plugin region's catcher.

`drive state` shows the window's size, scale and focus, whether it is fullscreen, the cursor's
shape, the pointer and the fingers down, held keys, an input method's composition, a back
gesture under way, files being dragged in, and any open file dialog.

## Targets

A command that acts on something takes a TARGET:

- `#TEST_ID`, such as `#checklist.add`;
- `X,Y` in the tree's pixels, or `X,Y,WIDTH,HEIGHT` for an area (for `shot`);
- text found in one line of the tree, such as `'"Send invitation"'` or `'Button "Create"'`. A
  match inside another match counts as that one, so a button and its own label are one target;
  when several unrelated lines match, the error lists them.

## Acting

`drive help` lists every command; these are the kinds:

```
drive click TARGET [right|middle|double]
drive move TARGET                         hover
drive down TARGET; drive pause 600; drive move TARGET; drive up   any gesture, step by step
drive drag FROM TO [STEPS]
drive wheel TARGET 3                      three mouse-wheel ticks down; a second number turns sideways
drive scroll TARGET 300                   a touchpad scroll of 300 pixels
drive zoom TARGET 2                       a touchpad pinch
drive leave                               the pointer leaves the window
drive tap TARGET                          touch, finger 0
drive swipe FROM TO
drive pinch TARGET 2                      two fingers spreading apart
drive touch down 0=TARGET 1=TARGET; drive touch move 0=... 1=...; drive touch up
drive back                                the system back gesture; back 0.4 holds it part way,
                                          back commit and back cancel finish it
drive type 'buy milk'                     text into what has focus; a newline presses Enter
drive key ctrl+z Enter shift+Tab          chords, each pressed and let go in turn
drive keydown alt; drive key tab tab; drive keyup alt
drive ime compose ni; drive ime commit 你  an input method's composition
drive act ACTION_ID [PANE]                run an action the way the command palette does
drive a11y TARGET [ACTION [VALUE]]        what a screen reader does: click, focus, increment,
                                          set VALUE, expand, scroll-down, ...
```

- `touch` names each finger (`0=TARGET`, `1=TARGET`) so several move in one frame, which is how
  to make a pinch of your own. `touch up` with no finger lifts them all.
- `keydown` on a key that is already down repeats it.
- `type` sends text, which a Wayland program's window does not take; `key` and `keydown` also
  send each key's scan code, which it does.
- `a11y TARGET` with no action prints the node a screen reader would act on and the actions it
  offers. Acting goes through the accessibility tree rather than the pointer or keys, the path a
  screen reader takes, and reaches plugins' nodes too.

### Scripts

`drive -` reads commands from standard input, one a line, quoted as a shell would, prints each
before its answer and stops at the first that fails:

```
drive - <<'EOF'
click "Add a root block"
click 'Label value="Checklist"'
wait 'Pane "Checklist"'
shot checklist.png 'Pane "Checklist"'
EOF
```

## Waiting and time

Every command that gives input waits until the app has settled, then prints what changed in the
tree as `-` and `+` lines, so there is no need to wait or read the whole tree again.

- A long change is cut to its first 60 lines; `drive --changes=N COMMAND` (or `all`) prints
  more.
- The app has settled once a frame asks for no other and `App::busy` is false. block-app is
  busy while a plugin is starting, owes a frame or holds unanswered input, while an account or
  workspace request is pending, and while the block store has commands to carry out or is still
  loading the workspace's graph.
- `drive --no-settle COMMAND` answers after the frame that took the input instead, to see the
  app part way through something, such as a plugin still loading.
- Each command gives up after 30 seconds, or `drive --timeout=SECONDS`.

| Command | Waits for |
|---|---|
| `drive wait TEXT` | a line of the tree to contain TEXT (or `#TEST_ID`): for work the app cannot see, such as a server push |
| `drive gone TEXT` | no line to contain it |
| `drive settle` | the app to settle |
| `drive pause MILLISECONDS` | time to pass by the app's own clock |

`drive clock stop` stops the app's clock: time then passes only with `drive pause`, a frame at a
time, so an animation can be stopped at any point and screenshotted the same way every run, and
an app that animates forever still settles. Anything else waiting on time, such as a plugin's
frame timeout, waits for `pause` too. `drive clock run` starts the clock again.

## The headless window

A headless program's window is simulated, and `drive` can do to it what a person does to a real
one:

| Command | Effect |
|---|---|
| `drive resize 390x844@3` | resizes it and sets its scale, to see a phone's layout (headless only) |
| `drive resize screen 1920x1080` | the screen it fills when the app asks for fullscreen |
| `drive focus`, `drive blur` | gives it focus or takes it away |
| `drive clipboard [TEXT]` | prints the clipboard, or sets it; the app's own copy and paste use it, and `drive key ctrl+v` pastes it; headless, it is the window's own, in a real window the system's |
| `drive upload FILE [TARGET]` | clicks TARGET and answers the file dialog it opens with FILE; without TARGET it answers one already open |
| `drive dismiss` | cancels a file dialog |
| `drive grab FILE...` | drags files in from outside, over the pointer; `drive move` carries them, `drive drop [TARGET]` drops them, `drive ungrab` takes them away |
| `drive close` | asks the window to close, as its close button does; the app exits, or stays open to ask about unsaved changes, and answers with what changed |

A file dialog waits for `drive` in any window while `drive` is attached, and `drive close` works
in a real window too.

## Driving any beui program

beui's run functions (`beui::run`, `run_with`, `run_with_renderers`, `run_on`) read two
variables, so a program needs nothing of its own to be driven:

- `BEUI_HEADLESS=WIDTHxHEIGHT[@SCALE]` runs it on `beui::Headless`
  (`crates/beui-adapter-headless`), which draws offscreen with wgpu and needs no display
  server. `BEUI_HEADLESS_SCREEN=WIDTHxHEIGHT` is the screen a fullscreen window grows to.
- `BEUI_AUTOMATION=PATH` answers `drive` on the Unix socket PATH. It works in a window too.

`be-drive launch` (`crates/be-drive`) sets both, waits for the program to settle and writes the
`env` file. `headless(...)` in `buck/app/drive.bzl` makes a crate's `:dev` target from it.

`be-drive run` takes the same options and is for tests. It starts the program with fresh data
and its output on its own, runs the commands on its standard input as `drive -` does, waits for
the program to exit, and fails unless every command succeeded and the program exited cleanly.
block-app's smoke test (`buck/app/smoke.sh`) is `wait` for its shell's pane, then `close`.

## How it works

`beui_core::app::automation` runs the commands inside the runner every adapter shares
(`guides/beui/apps.md`), so input lands on the queue a window's own events go to, and
everything above it (the document, block-app's plugin host, the Wayland compositor) runs as it
does with a window.

What a headless run never exercises is the platform underneath: winit's translation of the
system's keys, its clipboard and file dialogs, the window's swapchain and real input methods.
For those, run the program in a window (`guides/running/native.md`). The web build answers the
same commands through Playwright (`guides/running/web.md`).

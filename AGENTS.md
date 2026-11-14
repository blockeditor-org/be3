BE3 project

You are inside of an Ubuntu VM. You may install and remove programs as needed, and free up disk space if it runs out.

Guides:
- VISION.md: what the project is for, why it has the parts it has, and where each editor is headed
- guides/adding_a_block.md
- guides/adding_a_game.md
- guides/adding_an_app.md: writing an app (an editor): the package, the editor component, content and gestures, testing, the manifest, what an app may depend on, building
- guides/apps/editor.md: inside an editor's frame: the frame menu, chrome and side panels, phones, pan and zoom, making blocks and artifacts, presence, accessibility
- guides/apps/host_services.md: what an editor asks of the host: EditorHost calls and their answers, the graph, picking and renaming blocks, watches and host calls, extending the app protocol
- guides/apps/children.md: placing children: Subregion and ChildBlock, modes, a child that takes the frame, focus, docks that join across apps, program windows
- guides/apps/framework.md: be-sdk and be-sdk-beui: views, beui's app platform, windows, anchored views, input, fonts, where the pixels live
- guides/apps/runtime.md: how the host runs apps: staging, backends, frames and pacing, layout and present commits, when an app stops
- guides/bootstrap.md: crates/be-tool-bootstrap, the prebuilt tool every script runs, and how CI publishes and pins it
- guides/beui.md: what beui is, the essentials of writing components, the crate map and the two cores, and which guides/beui/ file covers what
- guides/beui/core.md: writing views, the frame, layout and measurement, coordinates, developing a base component, the HTML core and beui-html (strings, live DOM, hydration), testing beui
- guides/beui/components.md: the unstyled and styled component layers, tokens and theme colours, controls and anchored popups, scrolling and long lists, inputs and forms, drag and drop, touch, timers and animation, preferences, drawing with the gpu, writing unstyled and styled components
- guides/beui/text.md: the base and styled Text, layout scale and raster, fonts and Lucide icons, the text area editor, its looks and rich text, selecting text
- guides/beui/overlays.md: overlays and layers: the layer tree and stacking, anchors and placement, policies, dismissing and back, focus and layers, the lock screen, modals and dialogs, toasts, trees, claiming presses, layers that escape their window
- guides/beui/docking.md: the dock (beui-dock, beui-dock-host, beui-dock-state): declaring and reconciling, saving, panels and focus, modes, the window manager, nested docks, dialogs, tool bars, fullscreen; and windows and window hosts
- guides/beui/programs.md: a standalone beui program (Program, Root, features, RunOptions), the layer stack, drivers, the inspector and its simulation tools, and beui inside an app
- guides/beui/platforms.md: what runs under a beui program: the runner, window state and reports, unseen windows, input normalization, clipboard and paste, renderers, winit, the web, Android, headless, and displays under beui-platform-drm
- guides/beui/space.md: the Space VR simulator layer and OpenXR headsets (beui-layer-space, beui-layer-xr), controller lasers, and beui's on-screen keyboard
- guides/beui_keyboard.md: focus and input in beui: the keyboard conventions controls follow, how a key travels, actions and the command palette, forwarding to embedded editors and Wayland programs, the input method, touch
- guides/buck2.md
- guides/canvas.md: the infinite canvas app and its block type: the model and raster layers, profiles, line binding, hand-drawn and freehand drawing, gestures, device input, export, and its tests and bench
- guides/chat.md: the chat block: its header and segments, merging, the editor and sending state, notifications, unread counts
- guides/common_commands.md: for people too: running things as a person (a window, the VR simulator and an XR headset, the displays and the login session, an Android device) and as an agent (headless `:dev`, `drive`), and every buck2 target
- guides/build_server.md
- guides/frozen_formats.md: that no saved format is frozen yet, and how to freeze one later: the rules, and the tests that would hold it
- guides/hosting.md
- guides/desktop.md: be-host as a desktop, what every host shares: the `be3:desktop` package, DesktopHost and the mailbox, program windows as tabs, displays as one space, a shell that stops
- guides/desktop/shell.md: the desktop app: its bars per display, the Status bar and Quick settings, the Apps panel, window chips, window keys
- guides/desktop/linux.md: the Linux host: running it windowed or as a login session (`BEUI_PLATFORM=drm`), where the code is, displays and outputs, files opened in programs
- guides/desktop/wayland.md: be-host-wayland, the compositor: program windows, input, keyboard layouts and the on-screen keyboard, cursors, frame pacing, X11 programs, clipboard
- guides/desktop/login_session.md: the Linux login session: installing it, the log, panics, screens turning off, switching terminals, power and logging out, the lock screen, rotation and pointing devices, input remaps
- guides/desktop/linux_services.md: the D-Bus services in be-host-services: notifications, the tray, media and hardware keys, Wi-Fi
- guides/desktop/windows.md: the Windows host, its crates (be-host-win32, be-host-win32-capture, be-host-win32-input) and focus mode
- guides/pan_and_zoom.md: the camera an editor is handed by its host: fitting, gestures, who owns the camera, drawing and scaled items on a Canvas
- guides/parallel_agents.md: coordinating subagents, each in its own cloud session: starting them, landing their branches, verify and build server outages
- guides/performance.md: benchmarks, comparing builds, profiling with perf, keeping verify cheap on the build server (what tests and compiles cost in CPU, how to measure it), and ideas already set aside
- guides/app_libraries.md: the libraries apps are linked against, how they are linked and loaded, and the raw app interface
- guides/pinenote.md: be-host's session on the PineNote: e-paper, a display device that cannot draw, building for it
- guides/pinenote_ebc.md: hrdl's `rockchip_ebc` e-paper driver: where its sources are, its hints and waveforms, its ioctl header
- guides/reactive.md: the reactive graph under beui: handles and scopes, when effects run, zones, pausing, loading, block projections, and how beui binds it to nodes
- guides/repository.md: which crates are the product, tooling, examples, separate projects or unrelated
- guides/running/drive.md: how to run and test any program; read this first
- guides/running/native.md
- guides/running/web.md
- guides/running/local_buck2.md: building with buck2 on this machine when no build server is reachable
- guides/running/android.md
- guides/settings.md: user, block and organization settings: where they live, how they resolve, who may change and read them
- guides/sync.md: `be sync`, which keeps a folder or workspace and a directory on disk in step both ways
- guides/terminal.md: the terminal block, whose shell runs on a machine `be TARGET terminal-host` signs in from
- guides/testing_a_gui.md: editor tests (BeuiTest), paintings and accepting them, host tests, tests on both beui cores
- guides/the_new_block_stack.md: the `be-*` crates every block lives in: goals, layers, and an index of guides/blocks/
- guides/blocks/server.md: the storage crates, be-server (protocol, publishing, organizations, access, announcements, accounts), running a server, test harnesses and the fault relay
- guides/blocks/client.md: `Peer` and `Connection`, live sessions and ownership, the refs database, saving and the outbox
- guides/blocks/content_types.md: content traits, `be_block_model` documents, sequences, the stored form and how it changes, blobs, testing a type
- guides/blocks/host.md: how the host holds content: type registry, stale blocks, worker and replicas, offline and stopping, the graph mirror, metadata, create/import/export, undo
- guides/blocks/apps.md: blocks on the app protocol: content frames, state and change callbacks, the region driver, graph queries and commands, presence, peer messages, LiveText
- guides/blocks/version_control.md: repositories, checkouts, snapshots, conflicts as data, local ids
- guides/blocks/keys.md: keys, scopes, rotation, escrow, identity and pairing, contacts, what the server can and cannot do
- guides/blocks/workspaces.md: organizations, workspaces and the files pane, device profiles and host-applied settings, sessions and view state
- guides/tracker.md: the issue tracker in be-host, and `be`, the command line tool agents use to read and change it
- guides/type_modules.md: block types as app wasm modules: the host-to-type protocol (`be-sdk-type-api`) and the replica library types link (`be-sdk-type-guest`)
- plans/: one file per plan of work not built yet. Each starts with a `Depends on:` line naming the plans (or the steps of them) it needs first, or `none.`, so the plans form a DAG; keep it acyclic and current when a plan lands, splits or changes. plans/deferred/ holds plans and parts of plans set aside until the user picks them up: don't start them unasked.
- decisions/: one file per decision: what was decided, why, and what was rejected. Check it before re-proposing something it records as rejected.

Commands (builds run on the build server; see guides/buck2.md and guides/build_server.md):
- `./scripts/verify`: the check, and the compile check: always run it before you finish. It compiles every first-party target on Linux and the apps' wasm, applies autofixes, accepts new and changed snapshots, and runs every lint and test. Use it instead of cargo, which no longer builds the workspace. Use a 10-minute timeout. CI runs the same thing on pull requests and pushes what it changes.
  - Write code however is natural and let its autofixes tidy it; don't do these by hand. It formats code (including `view!` bodies), splits tests into one per file, and renames `foo/mod.rs` to `foo.rs`.
- `./scripts/test NAME`: runs the tests whose full names (module path and function, such as `media::tests::plays::plays`) contain NAME, so NAME may also be a module path like `media::tests`, and prints all their output, passing or failing. It finds their crates and runs each on the build server, natively or as wasm, and downloads only their output. `--update` accepts the paintings they make.
- `./scripts/buck run //crates/be-tool-local:bin -- rasterize CRATE_DIR/snapshots/NAME.paint OUT.png [FRAME|all]`: turns a painting, or frames of a recording, into a PNG to look at.
- `./scripts/buck run //crates/be-host:dev`: run be-host headless, signed in with a workspace open, then `source ~/.cache/be3/dev/env` and drive it with `drive` (guides/running/drive.md). `-c be3.apps=checklist,text` before the target stages only those apps, the workspace and its files pane, which downloads and builds less. beui-demo and be-launcher have a `:dev` too.
- `./scripts/buck run //crates/be-host:native`: build and run be-host natively in a window.
- `./scripts/test //crates/be-host-tests:test`: the host tests, which `./scripts/verify` runs too. Each starts be-host natively and headless with the apps it names in crates/be-host-tests/BUCK and the dev workspace open, drives it as `drive` does, compares what the whole window paints, and fails unless be-host exits cleanly (guides/testing_a_gui.md, 6).
- `./scripts/buck build //crates/be-host:web`: for web-specific changes; `./scripts/buck run //crates/be-host:web` serves it on http://127.0.0.1:8080 (guides/running/web.md).
- `./scripts/buck run //crates/be-host:android`: for Android-specific changes; builds the APK (guides/running/android.md).
- `./scripts/be Issues row BE-7K3Q2M`: read or change a tracker row (guides/tracker.md); needs `external/.tracker-credentials`, which `./scripts/be` fetches on first use.

Do:
- Use commit messages of the form `type: message`, ending with a Co-Authored-By line naming your model.
- When done, open a pull request on GitHub. Don't watch it, subscribe to it or check on it, and don't create routines or check-in timers.
- If a build fails because of the build server (it is down, refuses the key, or drops calls; guides/build_server.md), rerun the command once. If it fails again, stop working and ask the user how to continue.
- To stop a process, kill its PID, or use `pkill -x NAME` to match the process name. Never `pkill -f PATTERN` or `kill $(pgrep -f PATTERN)`: each command runs as `bash -c '<the command text>'`, so the pattern also matches the shell the command runs in, and pkill kills that shell and the rest of the command with it.
- When updating GitHub Actions workflows, remember that the new version will run on old PRs that don't have main's new changes applied.

Design principles:
- Do not edit this list.
- If a request seems to require violating one of these, confirm with the user using AskUserQuestion before going ahead.
- If a request asks for a feature that seems to violate standard conventions or user expectations, use AskUserQuestion to confirm.
- General:
  - One obvious way to do things: prefer one canonical API for a concept. Don’t add parallel forms that users must choose between and we must maintain.
  - Ignore backward compatibility for data: no format is frozen yet (guides/frozen_formats.md), so change any saved or serialized format (the server's data and backups, block files, block content, network requests) however a change needs, with no migrations, renames kept for old names or fallbacks for old data. The project is early and no real data is stored; whoever has old data deletes it.
  - Treat everything in the repo as in scope for any task. Fix bugs and missing features at their source (in beui, for example) rather than working around them, and update any guide you find out of date.
  - Do not edit files named README.md.
  - Write guides for agents. Add something to a guide only if a summary of that area written from scratch would include it. Put notes that only help humans in the PR description, the handoff message, or both.
  - Don't repeat in other files what AGENTS.md already says; every agent reads it at startup.
- Sharing:
  - No in-app reach to strangers: every sharing or messaging relationship starts with an out-of-band contact link.
- Testing:
  - Don't sleep in tests. Wait on an explicit signal, and read time from the frame clock (`ctx.now()` or `timer::now()`), never `Instant::now()`, so tests can advance time instead.
  - Add a painting that demonstrates each visual change or feature (see guides/testing_a_gui.md). If an existing painting already shows the change, that's enough.
- beui:
  - beui is retained-mode and fine-grained reactive, like SolidJS. It is not like React or an immediate-mode UI: a component runs once, and changes reach the UI through signals. Use that reactivity rather than working around it: push changes, never poll for them, and don't rebuild trees to update them.
  - Keep a full layout from scratch O(n) in the number of nodes. Incremental relayout may do less, but don't add anything that makes a full layout worse than linear.
  - Derive damage from the retained tree by identity, never by comparing paint output.
  - Add new beui components to beui-demo too.
  - Give styled components design tokens (a text size, a weight, a color role), never raw pixels or colors. Base components take concrete values and know nothing about the theme.
- Apps:
  - Keep the app protocol independent of any GUI framework: no beui-specific types or ideas in it. Any framework that can render offscreen and run in wasm (egui, for example) should be able to use it unchanged.
  - Pass textures through the app protocol without them leaving the GPU.
- GUI:
  - Use icons from the icon library in `beui_core::icons`. Where no icon fits or the library isn't usable, use no icon. Never use emoji or other Unicode symbols as icons. If the user gives you a Unicode icon, that is for illustration, don't use it directly. This rule only applies within the repository, so it doesn't apply to a claude artifact for example.
  - Make a scroll view fill its area edge to edge, and put padding inside it, around the content: `scroll(padding(content))`, not `padding(scroll(content))`.
- Build system (buck2):
  - Keep `./scripts/verify` and `./scripts/ci` to exactly one buck2 invocation each, so buck2 can run everything in parallel.
  - Write anything that runs locally after that invocation in Rust, not shell. Rust has fewer footguns, and it compiles remotely inside the same invocation at almost no cost.

In your handoff message, mention any of these that apply:
- Small issues you encountered, or small things you noticed that could make the code or application better.
- New principles or constraints in a user message that may deserve to be added to the design principles list.
- Existing code you noticed that violates a design principle.
- Anything that took several tries to figure out and could be clarified for future agents.

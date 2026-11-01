BE3 project

You are inside of an Ubuntu VM. You may install and remove programs as needed, and free up disk space if it runs out.

Guides:
- VISION.md: what the project is for, why it has the parts it has, and where each editor is headed
- guides/adding_a_block.md
- guides/adding_a_game.md
- guides/adding_a_plugin_editor.md
- guides/bootstrap.md: crates/bootstrap, the prebuilt tool every script runs, and how CI publishes and pins it
- guides/beui.md
- guides/beui_keyboard.md
- guides/buck2.md
- guides/common_commands.md: for people too: running things as a person (a window, the VR simulator and an XR headset, the displays and the login session, an Android device) and as an agent (headless `:dev`, `drive`), and every buck2 target
- guides/build_server.md
- guides/frozen_formats.md: the formats every later build must keep reading, how to change them, and the tests that hold them
- guides/hosting.md
- guides/desktop.md: block-app as a desktop, what every host shares: the desktop shell, the `be3:desktop` package and its watches, shown and hidden entries and the picker, displays as one space, a shell that stops
- guides/desktop/linux.md: the Linux host: be-wayland, the login session on the displays under DRM (`BEUI_PLATFORM=drm`), installing it, notifications, the tray, media keys, Wi-Fi, input remaps, the lock screen, Wayland and X11 programs
- guides/desktop/windows.md: the Windows host, its crates (be-win32, be-win32-capture, be-win32-input) and its design
- guides/pan_and_zoom.md: the camera a plugin editor is handed by its host
- guides/parallel_agents.md: coordinating subagents in worktrees on one cloud VM: landing, memory and disk limits, build server outages
- guides/performance.md: benchmarks, comparing builds, profiling with perf, and ideas already set aside
- guides/plugin_libraries.md: the libraries plugins are linked against, how they are linked and loaded, and the raw plugin interface
- guides/pinenote.md: block-app's session on the PineNote: e-paper, a display device that cannot draw, building for it
- guides/pinenote_ebc.md: hrdl's `rockchip_ebc` e-paper driver: where its sources are, its hints and waveforms, its ioctl header
- guides/reactive.md: the reactive graph under beui
- guides/repository.md: which crates are the product, tooling, examples, separate projects or unrelated
- guides/running/drive.md: how to run and test any app; read this first
- guides/running/native.md
- guides/running/web.md
- guides/running/android.md
- guides/sync.md: `be sync`, which keeps a folder or workspace and a directory on disk in step both ways
- guides/terminal.md: the terminal block, whose shell runs on a machine `be TARGET terminal-host` signs in from
- guides/testing_a_gui.md
- guides/the_new_block_stack.md: the `be-*` crates every block lives in
- guides/tracker.md: the issue tracker in block-app, and `be`, the command line tool agents use to read and change it
- guides/type_modules.md: block types as plugin wasm modules: the host-to-type protocol (`block-type-api`) and the replica library types link (`block-type-guest`)

Commands (builds run on the build server; see guides/buck2.md and guides/build_server.md):
- `./scripts/verify`: the check, and the compile check: always run it before you finish. It compiles every first-party target on Linux and the plugins' wasm, applies autofixes, accepts new and changed snapshots, and runs every lint and test. Use it instead of cargo, which no longer builds the workspace. Use a 10-minute timeout. CI runs the same thing on pull requests and pushes what it changes.
  - Write code however is natural and let its autofixes tidy it; don't do these by hand. It formats code (including `view!` bodies), deletes comments and doc comments, splits tests into one per file, and renames `foo/mod.rs` to `foo.rs`.
- `./scripts/test NAME`: runs the tests whose full names (module path and function, such as `media::tests::plays::plays`) contain NAME, so NAME may also be a module path like `media::tests`, and prints all their output, passing or failing. It finds their crates and runs each on the build server, natively or as wasm, and downloads only their output. `--update` accepts the paintings they make.
- `./scripts/buck run //crates/local-tools:bin -- rasterize CRATE_DIR/snapshots/NAME.paint OUT.png [FRAME|all]`: turns a painting, or frames of a recording, into a PNG to look at.
- `./scripts/buck run //crates/block-app:dev`: run the app headless, signed in with a workspace open, then `source ~/.cache/be3/dev/env` and drive it with `drive` (guides/running/drive.md). `-c be3.plugins=checklist,file-tree` before the target stages only those editors and the workspace, which downloads and builds less. beui-demo and be-launcher have a `:dev` too.
- `./scripts/buck run //crates/block-app:native`: build and run the native app in a window.
- `./scripts/test //crates/app-tests:test`: the app tests, which `./scripts/verify` runs too. Each starts the native app headless with the editors it names in crates/app-tests/BUCK and the dev workspace open, drives it as `drive` does, compares what the whole window paints, and fails unless the app exits cleanly (guides/testing_a_gui.md, 6).
- `./scripts/buck build //crates/block-app:web`: for web-specific changes; `./scripts/buck run //crates/block-app:web` serves it on http://127.0.0.1:8080 (guides/running/web.md).
- `./scripts/buck run //crates/block-app:android`: for Android-specific changes; builds the APK (guides/running/android.md).
- `./scripts/be Issues row BE-7K3Q2M`: read or change a tracker row (guides/tracker.md); needs `external/.tracker-credentials`, which `./scripts/be` fetches on first use.

Do:
- Use commit messages of the form `type: message`, ending with a Co-Authored-By line naming your model.
- When done, open a pull request on GitHub. Don't watch it, subscribe to it or check on it, and don't create routines or check-in timers.
- To stop a process, kill its PID, or use `pkill -x NAME` to match the process name. Never `pkill -f PATTERN` or `kill $(pgrep -f PATTERN)`: each command runs as `bash -c '<the command text>'`, so the pattern also matches the shell the command runs in, and pkill kills that shell and the rest of the command with it.
- When updating GitHub Actions workflows, remember that the new version will run on old PRs that don't have main's new changes applied.

Design principles:
- Do not edit this list.
- If a request seems to require violating one of these, confirm with the user using AskUserQuestion before going ahead.
- If a request asks for a feature that seems to violate standard conventions or user expectations, use AskUserQuestion to confirm.
- General:
  - One obvious way to do things: prefer one canonical API for a concept. Don’t add parallel forms that users must choose between and we must maintain.
  - Keep reading whatever a frozen format saved, forever (guides/frozen_formats.md: the server's data and backups, and text, folder and database blocks), and change one only by adding to it. Don't keep backward compatibility in any other serialization format or in network requests: the project is early, and users can delete that data.
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
- Plugins:
  - Keep the plugin protocol independent of any GUI framework: no beui-specific types or ideas in it. Any framework that can render offscreen and run in wasm (egui, for example) should be able to use it unchanged.
  - Pass textures through the plugin protocol without them leaving the GPU.
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

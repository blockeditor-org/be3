BE3 project

You are inside of an Ubuntu VM. You may install and remove programs as needed, and free up disk space if it runs out.

Guides:
- guides/adding_a_block.md
- guides/adding_a_game.md
- guides/adding_a_plugin_editor.md
- guides/beui.md
- guides/beui_keyboard.md
- guides/buck2.md
- guides/build_server.md
- guides/hosting.md
- guides/linux_desktop.md: block-app as a Linux desktop: the session, notifications, media keys, the lock screen, Wayland programs
- guides/pan_and_zoom.md: the camera a plugin editor is handed by its host
- guides/reactive.md: the reactive graph under beui
- guides/running/drive.md: running any beui app headless and driving it with `drive`, the way to see a change working
- guides/running/native.md: block-app's flags, and running it in a window with xdotool for platform-layer changes
- guides/running/web.md
- guides/running/android.md
- guides/testing_a_gui.md
- guides/the_new_block_stack.md: the `be-*` crates every block lives in

Commands (builds run on the build server; see guides/buck2.md and guides/build_server.md):
- `./scripts/buck run //:check`: fast compile check of every first-party target, native and wasm. Use this instead of cargo, which no longer builds the workspace.
- `./scripts/verify`: the full check. It applies autofixes, accepts new and changed snapshots, and runs every lint and test. Use a 10-minute timeout. CI runs the same thing on pull requests and pushes what it changes.
  - Write code however is natural and let its autofixes tidy it; don't do these by hand. It formats code (including `view!` bodies), applies fixable clippy lints, deletes comments and doc comments, splits tests into one per file, and renames `foo/mod.rs` to `foo.rs`.
- `./scripts/test NAME`: runs the tests whose full names (module path and function, such as `media::tests::plays::plays`) contain NAME, so NAME may also be a module path like `media::tests`, and prints all their output, passing or failing. It finds their crates and runs each the way its tests need, natively or as wasm. `--update` accepts the paintings they make.
- `./scripts/buck run //crates/block-app:dev`: run the app headless, signed in with a workspace open, then `source ~/.cache/be3/dev/env` and drive it with `drive` (guides/running/drive.md). beui-demo and be-launcher have a `:dev` too.
- `./scripts/buck run //crates/block-app:app`: build and run the native app in a window (guides/running/native.md).
- `./scripts/buck run //crates/block-app:smoke`: a bounded launch test; run it for changes that could affect native startup.
- `./scripts/buck build //crates/block-app:web`: for web-specific changes; `./scripts/buck run //crates/block-app:web-serve` serves it on http://127.0.0.1:8080 (guides/running/web.md).
- `./scripts/buck run //crates/block-app:android`: for Android-specific changes; builds the APK (guides/running/android.md).

Do:
- Use commit messages of the form `type: message`, ending with a Co-Authored-By line naming your model.
- When done, open a pull request on GitHub. Don't watch it, subscribe to it or check on it, and don't create routines or check-in timers.
- If you find yourself polling while waiting for a command to finish, run `./scripts/nopoll` in the foreground.
- When updating GitHub Actions workflows, remember that the new version will run on old PRs that don't have main's new changes applied.

Design principles:
- Do not edit this list.
- If a request seems to require violating one of these, confirm with the user using AskUserQuestion before going ahead.
- General:
  - Don't keep backward compatibility in serialization formats or network requests. The project is early, and users can delete their data; block-app's crash handler offers this automatically.
  - Treat everything in the repo as in scope for any task. Fix bugs and missing features at their source (in beui, for example) rather than working around them, and update any guide you find out of date.
  - Do not edit files named README.md.
  - Write guides for agents. Add something to a guide only if a summary of that area written from scratch would include it. Put notes that only help humans in the PR description, the handoff message, or both.
  - Don't repeat in other files what AGENTS.md already says; every agent reads it at startup.
- Testing:
  - Don't sleep in tests. Wait on an explicit signal, and read time from the frame clock (`ctx.now()` or `timer::now()`), never `Instant::now()`, so tests can advance time instead.
  - Add a painting that demonstrates each visual change or feature (see guides/testing_a_gui.md). If an existing painting already shows the change, that's enough.
- beui:
  - beui is retained-mode and fine-grained reactive, like SolidJS. It is not like React or an immediate-mode UI: a component runs once, and changes reach the UI through signals. Use that reactivity rather than working around it: push changes, never poll for them, and don't rebuild trees to update them.
  - Keep a full layout from scratch O(n) in the number of nodes. Incremental relayout may do less, but don't add anything that makes a full layout worse than linear.
  - Add new beui components to beui-demo too.
- Plugins:
  - Keep the plugin protocol independent of any GUI framework: no beui-specific types or ideas in it. Any framework that can render offscreen and run in wasm (egui, for example) should be able to use it unchanged.
  - Pass textures through the plugin protocol without them leaving the GPU.
- GUI:
  - Use icons from the icon library in `beui_core::icons`. Where no icon fits or the library isn't usable, use no icon. Never use emoji or other Unicode symbols as icons.
  - Make a scroll view fill its area edge to edge, and put padding inside it, around the content: `scroll(padding(content))`, not `padding(scroll(content))`.
- Build system (buck2):
  - Keep `./scripts/verify` and `./scripts/ci` to exactly one buck2 invocation each, so buck2 can run everything in parallel.
  - Write anything that runs locally after that invocation in Rust, not shell. Rust has fewer footguns, and it compiles remotely inside the same invocation at almost no cost.

In your handoff message, mention any of these that apply:
- Small issues you encountered, or small things you noticed that could make the code or application better.
- New principles or constraints in a user message that may deserve to be added to the design principles list.
- Existing code you noticed that violates a design principle.
- Anything that took several tries to figure out and could be clarified for future agents.

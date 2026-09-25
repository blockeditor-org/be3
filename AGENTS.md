BE3 project

You are inside of an ubuntu VM. You may install / remove programs as needed. If the disk runs out of space, you may free up space.

Guides:
- guides/adding_a_block.md
- guides/adding_a_game.md
- guides/adding_a_plugin_editor.md
- guides/beui.md
- guides/beui_keyboard.md
- guides/buck2.md
- guides/pan_and_zoom.md
- guides/reactive.md
- guides/testing_a_gui.md
- guides/the_new_block_stack.md

Do not:
- When making changes to serialization formats or network requests, do not consider backwards compatibility with existing clients or data. The project is still early, and it is fine to ask the user to delete all their data. The crash handler in block-app will offer this automatically.
- Do not use unicode symbols for icons, either use an icon library or no icon at all.
- Do not edit README.md. If it is out of date, you may say so in your handoff message.
- Don't use worktrees. If using subagents, run them sequentially rather than in parallel.
- Do not create routines. Do not subscribe to PRs. Do not set check-in timers.

Verification:
- `./scripts/buck`: buck2, which builds, lints and tests the workspace on BuildBuddy's remote workers; every command below is a buck2 target. It installs the pinned buck2 into the checkout the first time it runs. It needs a BuildBuddy API key: `BUILDBUDDY_API_KEY` in the environment, or the key in `.buildbuddy-api-key` at the root of the checkout or in `~/.config/be3/buildbuddy-api-key`. See guides/buck2.md for what it covers. It generates `third-party/rust/BUCK` and `buck/cargo/crates.bzl` from the Cargo.toml files whenever they change (git ignores both), and a crate's `BUCK` file takes its dependencies from the latter, so Cargo.toml is the only place a dependency is declared.
- `./scripts/buck run //crates/block-app:app`: builds the native app with every plugin beside it and runs it.
- `./scripts/buck run //:check`: Use this for fast compile feedback. It runs rustc's check pass over every first-party target through buck2 - the host's and the plugins' and games' wasm - and fails with the compiler's errors. Prefer this over cargo, which the workspace no longer builds with.
- `./scripts/buck run //:verify`: The full check. It applies every autofix, accepts new and changed snapshots, and runs all lints and tests. CI does the same on a pull request and pushes whatever it changes to the pull request's branch as a commit. Use a 10 minute timeout in the tool call arguments so it is less likely to convert itself to a background task.
  - A plugin's own tests are compiled to wasm and run through the plugin host on this machine, because they read and write the accepted paintings in `snapshots/`; `./scripts/buck run //:verify -- --plugin-tests` runs them. `block-editor-plugin` and `block-editor-beui` are in that run too, because the guest half of the plugin framework only exists on wasm. For faster feedback on one editor, run `./scripts/buck test //crates/editors/checklist:test`, adding `-- --env UPDATE_SNAPSHOTS=1` to accept its paintings.
  - It will autofix formatting, clippy fixable rules, and it will autofix to enforce project-specific rules: It will delete all code comments & doc comments, it will structure test folders & files to the project's one test per file standard, it will automatically move+rename mod.rs files to be in the parent folder named after the folder instead, and it will format the bodies of `view!` macro calls (rustfmt cannot, because the body is not Rust syntax).
- `./scripts/buck run //crates/block-app:android`: run this for changes that affect features specific to Android. It builds the APK and signs it into `target/android/block-app.apk`; `-- --install` installs it with adb and starts it.
- `./scripts/buck build //crates/block-app:web`: run this for changes that affect features specific to web. `./scripts/buck run //crates/block-app:web-serve` serves it, with be-server behind it, on http://127.0.0.1:8080.
- `./scripts/buck run //crates/block-app:smoke`: run this for changes that could affect native startup or runtime integration. It performs a bounded automated launch in a virtual display with isolated data.

Do:
- Use commit message format `type: message`. Include Co-Authored-By: (model name).
- When done, create a pull request on github for the change. Do not watch the pull request and do not check in on its status.
- In your handoff message, mention any small issues you encountered or small things you noticed that could make the code / application better.
- If you don't need tests in your search results, consider `grep --exclude-dir="tests"`
- If you find yourself polling waiting for a command to finish, run `./scripts/nopoll` in the foreground

Design principles:
- if a request appears to violate one of these, confirm using the question tool. do not edit this list.
- general:
  - we should never be sleeping in a test. if we need to wait for something, we need to find a way to wait for it without sleeping.
- beui:
  - beui is a retained-mode ui that you interact with using a solidjs-like reactive framework.
  - beui layout is O(n) or better on the number of nodes in the tree.
  - beui is a reactive framework; we should push changes, not poll for changes.
- block-app plugins:
  - the plugin protocol is framework-independent. we theoretically could use it with different GUI frameworks without modifying the plugin protocol.
  - the plugin protocol passes textures without them leaving the GPU.

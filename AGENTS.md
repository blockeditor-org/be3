BE3 project

You are inside of an ubuntu VM. You may install / remove programs as needed. If the disk runs out of space, you may free up space.

Guides:
- guides/adding_a_block.md
- guides/adding_a_game.md
- guides/adding_a_plugin_editor.md
- guides/beui.md
- guides/beui_keyboard.md
- guides/buck2.md
- guides/build_server.md
- guides/hosting.md
- guides/pan_and_zoom.md
- guides/reactive.md
- guides/running_on_android.md
- guides/running_the_app.md
- guides/running_the_web_app.md
- guides/testing_a_gui.md
- guides/the_new_block_stack.md

Verification:
- `./scripts/buck`: buck2, which builds, lints and tests the workspace on our build server (guides/build_server.md); every command below but `./scripts/verify` is a buck2 target. It installs the pinned buck2 into the checkout the first time it runs. It builds on Namespace, waking it with `nsc`, which needs Namespace's token: `BE3_NAMESPACE_TOKEN` in the environment, or the token in `.namespace-token.json` at the root of the checkout or in `~/.config/be3/namespace-token.json`. `BE3_BUILD_SERVER=blocks.pfg.pw`, `buildserver.pfg.pw` or `buildbuddy` builds on another server instead, with its key from `BE3_BUILD_SERVER_KEY`. See guides/buck2.md for what it covers. The rules for every crate, first- and third-party, take their dependencies and features from `buck/cargo/crates.bzl`, which is generated from the Cargo.toml files and checked in, so Cargo.toml is the only place a dependency is declared: after changing a Cargo.toml, run `./scripts/buck run //:buckify` (`./scripts/verify` does too).
- `./scripts/buck run //crates/block-app:app`: builds the native app with every plugin beside it and runs it.
- `./scripts/buck run //crates/block-app:dev`: starts the app in a virtual display, signed in with a workspace open, for you to drive with xdotool. See guides/running_the_app.md.
- `./scripts/buck run //:check`: Use this for fast compile feedback. It runs rustc's check pass over every first-party target through buck2 - the host's and the plugins' and games' wasm - and fails with the compiler's errors. Prefer this over cargo, which the workspace no longer builds with.
- `./scripts/verify`: The full check, as one buck2 command that builds the autofixes, lints and tests on the build server; the script then writes what they changed into the checkout. It applies every autofix, accepts new and changed snapshots, and runs all lints and tests. CI does the same through `./scripts/ci` on a pull request and pushes whatever it changes to the pull request's branch as a commit. Use a 10 minute timeout in the tool call arguments so it is less likely to convert itself to a background task.
  - A plugin's own tests are compiled to wasm and run through the plugin host, comparing against the accepted paintings in `snapshots/`; `./scripts/verify --plugin-tests` runs them on the build server and accepts what changed. `block-editor-plugin` and `block-editor-beui` are in that run too, because the guest half of the plugin framework only exists on wasm. For faster feedback on one editor, run `./scripts/buck test //crates/editors/checklist:test`, adding `-- --env UPDATE_SNAPSHOTS=1` to accept its paintings, or `-- --test-arg NAME` to run only the tests whose names contain NAME (a bare argument after `--` is an error).
  - It will autofix formatting, clippy fixable rules, and it will autofix to enforce project-specific rules: It will delete all code comments & doc comments, it will structure test folders & files to the project's one test per file standard, it will automatically move+rename mod.rs files to be in the parent folder named after the folder instead, and it will format the bodies of `view!` macro calls (rustfmt cannot, because the body is not Rust syntax).
- `./scripts/buck run //crates/block-app:android`: run this for changes that affect features specific to Android. It builds the APK and signs it into `target/android/block-app.apk`; `-- --install` installs it with adb and starts it. See guides/running_on_android.md.
- `./scripts/buck build //crates/block-app:web`: run this for changes that affect features specific to web. `./scripts/buck run //crates/block-app:web-serve` serves it, with be-server behind it, on http://127.0.0.1:8080.
- `./scripts/buck run //crates/block-app:smoke`: run this for changes that could affect native startup or runtime integration. It performs a bounded automated launch in a virtual display with isolated data.

Do:
- Use commit message format `type: message`. Include Co-Authored-By: (model name).
- When done, create a pull request on github for the change. Do not watch the pull request and do not check in on its status.
- In your handoff message, mention any small issues you encountered or small things you noticed that could make the code / application better.
- If you don't need tests in your search results, consider `grep --exclude-dir="tests"`
- If you find yourself polling waiting for a command to finish, run `./scripts/nopoll` in the foreground
- When updating github actions workflows, remember that the new version will run on old PRs that don't have main's new changes applied

Do not:
- Do not create routines. Do not subscribe to PRs. Do not set check-in timers.

Design principles:
- do not edit this list.
- if it seems like a request requires violating one of these, confirm using the question tool.
- general:
  - when making changes to serialization formats or network requests, do not consider backwards compatibility with existing clients or data. te project is still early, and it is fine to ask the user to delete all their data. the crash handler in block-app will offer this automatically.
  - we should never be sleeping in a test. if we need to wait for something, we need to find a way to wait for it without sleeping.
  - guides and markdown files are for agents to read to help them understand the codebase and implement features. not every tiny change deserves a mention in a guide. when adding something to a guide, consider if a summary written from scratch would include the feature. if it wouldn't, don't add it to the guide. information that is only helpful to humans also doesn't belong in a guide. that can go in a PR description and/or handoff message.
  - every agent immediately, automatically reads AGENTS.md when it starts up. do not duplicate information that is already in AGENTS.md in other files.
  - do not edit any file named 'README.md'. if one is out of date, you may say so in your handoff message.
  - everything in the repo is in scope for editing for any task. for example, if you find a bug or missing feature in beui, fix it at the source rather than working around it. if you find a guide out of date or inaccurate, update it.
- beui:
  - beui is a retained-mode ui that you interact with using a solidjs-like reactive framework.
  - beui layout is O(n) or better on the number of nodes in the tree.
  - beui is a reactive framework; we should push changes, not poll for changes.
  - code that reads the time uses the frame clock (`ctx.now()` or `timer::now()`), not `Instant::now()`, so tests can advance it without sleeping.
  - new components added to beui should also be added in beui-demo
- block-app plugins:
  - the plugin protocol is framework-independent. we theoretically could use it with different GUI frameworks without modifying the plugin protocol.
  - the plugin protocol passes textures without them leaving the GPU.
- gui:
  - we use an icon library for icons. if one is not available, then do not use icons. do not use unicode for icons.
  - a scroll view reaches the edges of the area it fills; its padding goes inside it, around the scrolled content, so content is not cut off short of the edge and there is no padding right of the scrollbar / left of the scroll area.

In your handoff message:
- If any, mention any small issues you encountered or small things you noticed that could make the code / application better.
- If any, mention any new principles / constraints in a user message that may deserve to be added to the design principles list.
- If any, mention any existing code you noticed that is violating a design principle.
- If any, things that took a few tries to figure out and it could help future agents to clarify.

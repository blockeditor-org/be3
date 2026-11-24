# One way each in the tools

Depends on: none. Steps 2 and the bootstrap's part of 6 need a bootstrap release and a new pin
(guides/bootstrap.md).

## Steps

1. **`stress` reuses the runner and the comparison.** be-tool-local's `stress.rs` copies
   paint-compare's `run()` as `changed_paintings()`, and its `once()`/`wait()` enforce a time
   limit by killing only the child, so a run that times out can leave its hosts running, which
   be-tool-run-test (`--time-limit`, `--paintings`, a fresh TMPDIR, stopping the whole process
   group) does not. Its `ENVIRONMENT` list copies `run_env` from be-host-tests' BUCK by hand
   ("Nothing checks the two lists agree"). `stress` calls `be_tool_paint_compare::run` as a
   library and runs each attempt through run-test, built in the same buck invocation, which
   also hands it the test's environment. It stays local
   (decisions/host_tests_stay_flake_free.md).
2. **One acceptance of paintings.** be-tool-verify's `Run::paintings` copies a painting from
   `changed/` into `snapshots/` only when `used/NAME`'s hash matches the accepted file, and
   otherwise reports it stale; the bootstrap's `test.rs` `report()` (`./scripts/test --update`)
   copies without that check, so it can overwrite a painting edited after its test compared it.
   One `accept(package, record_dir) -> Accepted { changed, stale }` in a small library both
   use.
3. **One Playwright.** buck/tools/BUCK pins Node Playwright and chrome-headless-shell for the
   `.mjs` drivers in buck/program/; be-tool-browser-rasterizer instead runs `render.py` on
   Python Playwright from pip, which needs a venv, `BE3_PLAYWRIGHT_PYTHON` and a step of its own
   in `.github/workflows/ci.yml`. Port `render.py` to a `render.mjs` on the pinned Node
   Playwright, and drop the venv, the variable and the CI step. The `playwright()` locator and
   the static server copied in `page-test.mjs`, `web-test.mjs` and `web-host-test.mjs` move into
   one module beside `drive-words.mjs`. Accept the browser previews' paintings once if the
   versions draw them differently.
4. **One build-helper protocol.** be-tool-build-helper's `products.rs` (`Product`, `State`,
   `Status`, `Manifest`, `ManifestFile`, `PLATFORMS`) and be-tool-launcher's `helper.rs`
   (`Product::name`, `platform()`, `State` with `Refused`/`Unknown`, `Status`, `Manifest`,
   `File`) each define the wire. A serde-only `be-tool-build-helper-protocol`, as
   be-tool-paint-review-manifest is for paint review; the launcher's extra states become
   client-side outcomes.
5. **One painting kind.** beui-paint-snapshot's `accepted.rs` `changed()`, be-tool-local's
   `rasterize.rs` and `previews.rs`'s `ReviewSnapshot` each branch on `is_browser`, and their
   descriptions differ. A `Painting { Native, Browser }` in beui-paint-snapshot with `decode`,
   `frames`, `difference` and `render(index, browser_renderer)`; the others call it.
6. **One storage client and one copy of the small helpers.** be-tool-local's `deploy.rs`
   (ureq, a `Checksum` header, `stored_sha256`) and be-tool-build-helper's `host.rs` (curl by
   content hash) share one blocking client: put if absent by sha256, get, head. be-server's
   backup keeps its own, and CI's upload step stays separate as the only step that holds the
   key. About nine sha256-to-hex helpers across the tool crates use the `hex` crate already in
   Cargo.lock through one function, and be-tool-verify's and be-tool-local's `git()` and
   `git_bytes()` become one.
7. **One gate and one disk in servers/.** `gate.html` and `disk.nix` are near copies in
   servers/api and servers/build (labels, the cookie name and the disk id differ). Each becomes
   one function in `common.nix` taking those.
8. **Names that say what they are.** be-tool-local's Cargo description stops claiming it runs
   verify (be-tool-verify does), and its binary's name matches the crate the guides name.

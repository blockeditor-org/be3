# Running the web build

Check a change natively first (guides/running/drive.md); it is quicker. For a change that only
shows in the browser, serve the web build, open it in headless Chromium and drive the page:

    ./scripts/buck run //crates/be-host:web-dev
    source ~/.cache/be3/web-dev/env

This builds the web bundle with every app, starts be-server and Caddy in front of it on
http://127.0.0.1:8090, opens the page in headless Chromium signed in as `dev@localhost`
(`dev-password`) with the personal organization on My files, and returns once the host is up.
Data and the browser profile live in `~/.cache/be3/web-dev` and survive restarts; running it
again reopens the same organization, which is how to pick up a rebuild.

- `-- --fresh` deletes the data and browser profile first; `-- --stop` stops everything.
- `-- --webgl` starts the browser without WebGPU, so everything draws with WebGL.
- `BLOCK_WEB_DEV_DIR` moves the directory; `BLOCK_WEB_DEV_PORT` moves the page, and the server,
  debugging and Caddy admin ports take the three after it, for running two at once.
- It needs Node and Playwright's Chromium (`npm install -g playwright`,
  `npx playwright install chromium`).
- The page's and its workers' console goes to `browser.log` (`INFO:CONSOLE` lines), the
  server's to `server.log`.

The launcher opens `/?dev-workspace&automation`, and both parameters work on any page serving
the bundle: `dev-workspace` signs in through the ordinary portal against the page's own server
(the launcher completes the portal flow; by hand you would), and `automation` answers `drive`
through the bundle's `automate` export.

## Driving the page

`drive` answers the same commands as natively, run by the host in the page, except the headless
window's: the browser is the platform here, and these take their place:

- `drive shot FILE [TARGET]`: a screenshot through Playwright.
- `drive upload FILE TARGET`: clicks TARGET and answers the file chooser.
- `drive size W H` resizes the page; `drive reload` reloads it and waits for the host.
- `drive eval EXPRESSION`: JavaScript in the page.

`shot` and `upload` reveal their target and aim at the part that shows, with the same lookup
as the browser tests (`shownPlaces` in buck/program/drive-words.mjs). Tree coordinates are CSS
pixels at the launcher's device pixel ratio of 1.

`drive shot` captures the apps' canvases reliably only with `--webgl`; with WebGPU in the
headless browser it can time out once an app canvas shows.

## beui's HTML core in a browser

A crate on beui's HTML core (guides/beui/core.md, Cores) renders DOM elements instead of
drawing. Its bundle answers drive's reading commands (`tree`, `ids`, `find`, `reveal`, `shown`,
`wait`, `gone`, `settle`, `pause`, `clock`) through `beui::html::automate`, in the native line
format; input comes from Playwright. crates/beui-tests' `:web-test` drives its scripts that way
(guides/testing_a_gui.md, 7). A bundle on this core stages beui's fonts
(`//crates/beui-fonts:*.ttf`) beside its index.html, where its `@font-face` rules look.

- `./scripts/buck run //crates/beui-demo:web-html` serves the demo on this core
  (crates/beui-demo-html) on http://127.0.0.1:8075; `./scripts/test //crates/beui-demo-html:web-test`
  drives every page in Chromium.
- crates/be-server-portal-ui is be-server's account portal, rendered per request on the host
  (`beui::html::render_to_string`) and hydrated in the browser (`beui::html::hydrate`) from the
  `:assets` be-server's executable serves (guides/hosting.md).

## Apps on the page

Each app runs in a Web Worker of its own with its own GPU device: be-wasm-guest-host's worker
runtime (src/guest.js, which runs type modules too), crates/be-host/web/app.js, and
crates/be-wasm-gpu-shim for the gpu ABI. Nothing an app draws leaves its worker; the page
hands workers canvases instead (crates/be-host-runtime/src/web):

- Each shown screen is a `<canvas>` in `#apps`, behind the host's canvas `#block`, transferred
  as an `OffscreenCanvas` and placed by CSS transform, `clip-path` and `z-index`
  (`web/canvases.rs`); the host's canvas punches a hole where it shows.
- An app with an anchored view (crates/be-sdk-api/PROTOCOL.md, Anchored views) gets one overlay
  canvas in `#layers` above `#block`, with `pointer-events: none` (`web/overlay.rs`); input
  still lands on the host's canvas, which routes it. Scrims are drawn by the host, below every
  overlay. A child placed in an anchored view shows under it on the web, since child screens
  are behind `#block`.
- Screens and overlays are placed from the host's last frame, so on the web they follow the host
  by one frame.

## Files and offline

The web build keeps the same files as native be-host, under `/be-host` in the origin private file
system. WASI file calls go from `web/wasi.js` to `web/files.js`, a worker that owns the sync
access handles; without OPFS (some private windows) it answers from memory and nothing
survives a reload. Traps that shape this code:

- SQLite's WASI build locks with a `NAME.lock` directory, which a reload mid-transaction leaves
  behind, so `be_block_sqlite_schema::open` uses the `unix-none` VFS on WASI; the `block-files`
  Web Lock keeps one tab as owner (a second tab waits for it).
- The block stack (`be_host_stack::start`) runs in its own worker, `web/be-worker.js`: a second
  instance of the module on the page's shared memory, with its own sockets and timers. A browser
  traps `memory.atomic.wait` on the page's thread, so a lock shared with that worker is
  `be_wasm_threads::Mutex`, which spins. A server connection's `Peer` locks a
  `std::sync::Mutex`, so connections never cross between the page and the worker
  (`be_host_stack::link` keeps them per thread on wasm).
- An unload cannot wait for the worker, so durability does not depend on it: a replica seals
  within `APPEND_INTERVAL` of an edit (guides/blocks/client.md, "The journal"), `beforeunload` asks while anything is unsaved
  (`be_host_stack::status().unsaved`), and the worker seals while the browser asks.
- Every context counts the monotonic clock from `performance.timeOrigin`, so an `Instant` means
  the same time in the page and the worker.
- `web/sw.js`, the service worker, keeps what the page fetched (network first, except `/api` and
  `/auth`; a network silent for 3 s gets the kept answer), so a page opened once opens offline.
- Push notifications (plans/notifications.md): the browser decrypts a Web Push payload before
  `sw.js` sees it, so the worker holds a `PushMessage`'s bytes, never a key. It asks every open
  window to open it (`web/push.js`, bound in `host.js` to the module's `push_open`, which runs
  `Peer::open_notification` and names the sender and block from the host's contacts and graph,
  `be-host/src/web_push.rs`), shows the answer for up to 3 s, else "New message", and opens a
  repeat (a hash of the sealed bytes, kept in IndexedDB with only the block it was about,
  never its text) never: what is on screen stays, and a repeat of one dismissed or pressed
  comes back as "New message" without a sound. `globalThis.bePush.enable()`
  asks permission, subscribes and registers the endpoint (call it from a gesture); `disable()`
  and signing out unregister. The switch is the settings list's "Notifications" row, which asks
  the host through `set_push_device` (`be_host_base::push_device`, installed by `web_push::install`
  and published as `PushDeviceWatch`). A pressed notification focuses a window and posts the
  block to it (`push_open_block`), or opens `?open-block=ORGANIZATION/BLOCK`; a block of another
  organization first opens that organization, unless changes are unsaved. An endpoint is
  registered per organization, so the host registers the browser's subscription again
  (`bePush.sync`) each time it connects to an organization with pushes on, which also finds a
  subscription the push service replaced (`pushsubscriptionchange`, which `sw.js` renews).
  Signing out unregisters the endpoint and drops the browser's subscription only when no
  saved account is left; another account removed while one is open keeps its endpoint until a
  push to it fails. `//crates/be-host:push-web-test` runs the real
  `sw.js` and `push.js` in Chromium with pushes delivered through DevTools
  (`deliverPush` in buck/program/page-test.mjs); headless Chromium never grants the permission to
  show a notification, so the test's worker (`web-test/sw.js`) reports what `sw.js` would show,
  moves its clock on, and raises the press and subscription-change events.
- The host closes its tab when it exits, unless the page is already leaving.

Typing offline then reloading is checked by hand: open a text block, `kill -STOP` the
instance's be-server (or Playwright's `context.setOffline(true)`), type with
`page.keyboard.type`, `page.reload()` with a `dialog` handler that accepts the `beforeunload`
prompt, and read the editor's value from `drive tree`. Compare `performance.timeOrigin` to be
sure the page reloaded, and `kill -CONT` the server before stopping web-dev.

## Hosted web build

The deployed bundle lives at `https://be3.pfg.pw/data/BUILD/web/index.html`; `/web` loads the
build matching the deployed server while keeping the address at `/web`, so relative asset URLs
resolve against a document base pointing at the build directory, and page navigation resolves
against the page URL instead. Add account offers the deployed `build-info.json`'s default
server; local and CI builds have none. Bunny's setup is in guides/hosting.md.

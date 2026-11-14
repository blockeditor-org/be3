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
  debugging and Caddy admin ports take the three after it, for running two at once. All of
  them listen on 127.0.0.1 only.
- It needs Node and Playwright's Chromium (`npm install -g playwright`,
  `npx playwright install chromium`).
- The page's and its workers' console goes to `browser.log` (`INFO:CONSOLE` lines), the
  server's to `server.log`.

The launcher opens `/?dev-workspace&automation`, and both parameters work on any page serving
the bundle: `dev-workspace` signs in through the ordinary portal against the page's own server
(the launcher completes the portal flow; by hand you would), and `automation` answers `drive`
through the bundle's `automate` export.

`./scripts/buck run //crates/be-host:web` serves the bundle with every app and a be-server
behind it on http://127.0.0.1:8080 without a browser (`-- --backend HOST:PORT` moves the
server). be-server's `--auth-origin` is `http://` and the `--addr` given, so sign in from
http://127.0.0.1:8080, not localhost. It and every other web bundle `./scripts/buck run` serves
(`//crates/beui-web-demo:web`, `//crates/beui-demo:web-html`,
`//crates/be-tool-paint-review-web:web`) listen on loopback alone; `-- --addr HOST:PORT` moves
them, and `-- --addr 0.0.0.0:PORT` is how to open one from another machine. A browser hands
shared memory only to a secure page, which plain http is only on localhost, so be-host's page
served elsewhere needs `--domain`. Caddy serves them, which binds every interface unless its
Caddyfile says otherwise: guides/hosting.md, "Listening", has the rule and the check that holds
it.

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
drawing. Its bundle answers drive's reading commands (`tree`, `ids`, `find`, `wait`, `settle`, `pause`, `clock`) through `beui::html::automate`, in the native line
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

- Each shown screen is a `<canvas>` in `#apps`, behind the host's canvas `#host`, transferred
  as an `OffscreenCanvas` and placed by CSS transform, `clip-path` and `z-index`
  (`web/canvases.rs`); the host's canvas punches a hole where it shows.
- An app with an anchored view (crates/be-sdk-api/PROTOCOL.md, Anchored views) gets one overlay
  canvas in `#layers` above `#host`, with `pointer-events: none` (`web/overlay.rs`); input
  still lands on the host's canvas, which routes it. Scrims are drawn by the host, below every
  overlay. A screen over an anchored view painted before it (a child placed in one) goes into
  `#layers` too, stacked with the overlays by paint order, so it shows above the view as it does
  natively; every other screen stays in `#apps`, under the host's chrome (`presenter::lifted`).
  Screens and overlays carry `data-kind="screen"` or `"overlay"`.
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
  instance of the module on the page's shared memory, with its own sockets and timers. Each
  stack it starts waits for the previous one's end (a oneshot, `web.rs`), never polls for it.
- Nothing may spin or poll while it waits; a worker waits with `Atomics.wait` (std's locks and
  `Condvar`, `web/wasi.js`'s file calls, `web/dylink.js`'s lock), woken by the side that
  changed the state. A browser traps `memory.atomic.wait` on the page's thread, so the page
  may never wait at all. A module's page entry calls `be_wasm_threads::initialize_page` and a
  worker's `initialize_worker`; `be_wasm_threads::may_wait` tells them apart. Two waits on the
  page remain until plans/web_page_never_waits.md lands: `be_wasm_threads::Mutex::lock`, for
  the locks the page shares with the stack's worker, spins on the page (and waits for real
  everywhere else), and the page's file calls spin until `files.js` answers. Don't add a
  third: a page that needs something another thread holds asks for it with a message.
- A server connection's `Peer` locks a `std::sync::Mutex`, so connections never cross between
  the page and the worker (`be_host_stack::link` keeps them per thread on wasm).
- An unload cannot wait for the worker, so durability does not depend on it: a replica seals
  within `APPEND_INTERVAL` of an edit (guides/blocks/client.md, "The journal"), `beforeunload` asks while anything is unsaved
  (`be_host_stack::status().unsaved`), and the worker seals while the browser asks.
- Every context counts the monotonic clock from `performance.timeOrigin`, so an `Instant` means
  the same time in the page and the worker.
- `web/sw.js`, the service worker, keeps what the page fetched (network first, except `/api` and
  `/auth`; a network silent for 3 s gets the kept answer), so a page opened once opens offline.
- Push notifications (plans/notifications.md). The browser decrypts a Web Push payload before
  `sw.js` sees it, so the worker holds a `PushMessage`'s bytes, never a key.
  - *Showing.* The worker asks every open window to open the push (`web/push.js`, bound in
    `host.js` to the module's `push_open`, which runs `Peer::open_notification` and names the
    sender and block from the host's contacts and graph, `be-host/src/web_push.rs`) and shows the
    first answer, or "New message" after 3 s, with no window or without the key.
  - *Repeats.* A hash of the sealed bytes and the block it was about, never its text, is kept
    in IndexedDB. A repeat is not opened again: what is on screen stays, and one dismissed or
    pressed comes back as "New message" without a sound. The store is deleted with the
    subscription.
  - *The switch.* `globalThis.bePush.enable()` asks permission, subscribes and registers the
    endpoint (call it from a gesture); `disable()` unregisters it. The settings
    list's "Notifications" row asks the host through `set_push_device`
    (`be_host_base::push_device`, installed by `web_push::install` and published as
    `PushDeviceWatch`).
  - *Registering.* An endpoint is registered per organization, so the host registers the
    browser's subscription again (`bePush.sync`) each time it connects to an organization with
    pushes on, which also finds a subscription the push service replaced
    (`pushsubscriptionchange`, which `sw.js` renews).
  - *Signing out.* Logging an account out (`web_push::sign_out`) sends the browser's endpoint
    with `logout`, so the server forgets it for that account in every organization, with no
    organization open; the browser's subscription is dropped only when no saved account is left.
  - *Pressing.* A pressed notification focuses a window and posts the block to it
    (`push_open_block`), or opens `?open-block=ORGANIZATION/BLOCK`, which any link can do. A block
    of another organization first opens that organization, unless changes are unsaved; the host
    holds it until the organizations, the graph or the unsaved changes change, and drops it
    after 30 s. A worker killed before a loading page says it is ready (30 s) loses a press.
  - *Tests.* `//crates/be-host:push-web-host-test` runs the whole page (`web_host_test`, below):
    Ada writes in a chat Dev shared while Dev's tab is closed, be-server pushes it to Dev's
    endpoint, and Dev's worker shows it named, once, or "New message" without a tab or the key;
    signing out forgets the endpoint. `//crates/be-host:push-web-test` runs the real `sw.js` and
    `push.js` beside a page standing in for the host, with pushes delivered through DevTools
    (`deliverPush` in buck/program/page-test.mjs); its worker (`web-test/sw.js`) reports what
    `sw.js` would show, moves its clock on, and raises the press and subscription-change events.
- The host closes its tab when it exits, unless the page is already leaving.

`web_host_test` (buck/program/drive.bzl) tests the whole page: `//crates/be-host:web-host-test`
serves `:web` through caddy with a fresh be-server behind it, signs dev@localhost in through the
portal (buck/program/web-sign-in.mjs, which registers it first), sizes the page to 960×600 by
resizing it (the host takes its window's size from a resize, as `drive size` gives it; the
harness's `VIEWPORT`, kept small because SwiftShader draws every pixel on the CPU, guides/performance.md,
Keeping verify cheap), waits for the
workspace, and hands a script in crates/be-host/web-test/ `drive(...)`, answered like `drive` by
the page's `automate`, with the Playwright page beside it; `test.account(name)` signs another
account in, in a browser context of its own. A failure prints the host's tree and the ends of the
server's and caddy's logs.

The harness stands in for the push service, which headless Chromium lacks along with the
permission to show notifications: each page's `PushManager` subscribes to an endpoint the
harness serves and decrypts (RFC 8291) what be-server posts to it (`pushes.next()`), the worker's
`showNotification` and `getNotifications` keep what it shows (`notifications()`), and
`deliverPush` dispatches a push in the worker, since DevTools' `deliverPushMessage` carries text,
not a push's bytes, and needs an open page. buck/program/web-host-test.mjs lists the rest.

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

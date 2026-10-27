# Running the web app

For a change that only shows in the browser, serve the web build and open it in a headless
Chromium, then drive the page from the shell (anything else is checked natively, which is
quicker: guides/running/drive.md):

    ./scripts/buck run //crates/block-app:web-dev
    source ~/.cache/be3/web-dev/env

The command builds the web bundle with every plugin and starts be-server and Caddy in front
of it on http://127.0.0.1:8090. It then opens the page in a headless Chromium and returns
once the app is up. The page is signed in to `dev@localhost` (password `dev-password`) on
that be-server, with the personal organization open on My files, so there is no account or
organization to make first. The server's data and the browser's profile live in `~/.cache/be3/web-dev` and
survive restarts, so running the command again reopens the same organization, which is how to
pick up a rebuild.
- `-- --fresh` deletes the data first.
- `-- --webgl` starts the browser without WebGPU, so the app and its plugins draw with WebGL.
- `-- --stop` stops the browser and the servers.

`BLOCK_WEB_DEV_DIR` moves the directory. `BLOCK_WEB_DEV_PORT` moves the page, and be-server,
the browser's debugging port and Caddy's admin port take the three ports after it, for running
two at once.

It needs Node and Playwright's Chromium: `npm install -g playwright` and
`npx playwright install chromium` where they are missing.

`env` defines `drive`, which runs one command against the page and exits. Everything the page
and its plugins' workers log to the console goes to `~/.cache/be3/web-dev/browser.log` as
`INFO:CONSOLE` lines, among the browser's own output. be-server's goes to `server.log`.

What the launcher opens is `http://127.0.0.1:8090/?dev-workspace&automation`; those
two parameters work on any page serving the bundle:
- `dev-workspace`: start browser sign-in against the page's own server, then open the
  last organization or the personal organization. The launcher drives the ordinary
  portal as `dev@localhost`, reads its verification message from the development server
  log when registering, and explicitly approves the device request. The parameter
  alone requires completing that portal flow manually.
- `automation`: answer `drive`'s commands through the bundle's `automate` export.

## Driving the page

`drive` answers the native app's commands (guides/running/drive.md), run by the app itself in
the page under one drive layer, except those of the headless window: the browser is the
platform here. Its own commands take their place (`drive` with no command lists them):
- `drive shot FILE [TARGET]` saves a screenshot through Playwright, of TARGET only when given.
- `drive upload FILE TARGET` clicks TARGET and answers the file chooser that opens with FILE.
- `drive size W H` resizes the page. `drive reload` reloads it and returns once the app answers
  again; any command waits for the module to start, so one sent mid-reload is not lost.
- `drive eval EXPRESSION` evaluates JavaScript in the page and prints the result.

The tree's coordinates are the page's CSS pixels when the device pixel ratio is 1, as it is in
the launcher's browser.

## beui's HTML core in a browser

A crate built on beui's HTML core (plans/html_core.md) runs its pages as DOM elements rather
than drawing them. Its bundle answers the reading commands of `drive` through
`beui::html::automate(words)` (beui-core-html's `automate`): `tree` and `ids` in the native
app's line format, `find`, `wait`, `gone`, `settle`, `pause` and `clock stop|run`. Input does
not go through it: the browser gives it, from Playwright. crates/beui-shared-tests' `:web-test`
does that for its scripts (guides/testing_a_gui.md, 7), with buck/app/web-test.mjs, which
shares its command parsing with web-drive.mjs (buck/app/drive-words.mjs).

crates/beui-html-spike is block-app's account picker (crates/block-accounts-ui) on that core,
rendered on the host and hydrated in the browser: `./scripts/buck run //crates/beui-html-spike:web`
serves it on http://127.0.0.1:8060 (or the address after `--`). Its `:ssr` program writes the
page at build time with `beui::html::render_to_string`; the module calls `beui::html::hydrate`.
`./scripts/test //crates/beui-html-spike:web-test` drives it in Chromium and checks that
hydration kept every prerendered element and, in both colour schemes, changed no colour. A bundle on this core stages the bundled fonts
(`//crates/beui-fonts:Inter-Regular.ttf`, `Hack-Regular.ttf`, `NotoEmoji-Regular.ttf` and
`lucide.ttf`) beside its index.html, where the core's `@font-face` rules look for them.

beui's demo pages run on that core too: `./scripts/buck run //crates/beui-demo:web-html` serves
crates/beui-demo-html on http://127.0.0.1:8075, a catalog beside the page it shows instead of the
native demo's dock. beui-demo-html builds beui-demo without its `native` feature, which holds
the dock shell and the samples only the native core can show (docking, the on-screen keyboard,
windows, paths); those show a line saying so. `./scripts/test //crates/beui-demo-html:web-test`
opens every page in Chromium.

## Files and offline

The web build keeps the same files as the native app, under `/block` in the
origin private file system: `app.sqlite3`, the refs database and `be-objects`.
The module reaches them through ordinary WASI file calls; `web/wasi.js`
forwards every call on a descriptor other than the standard streams to
`web/files.js`, a worker that owns the files' sync access handles and reads
and writes the module's shared memory itself. A worker caller waits on
`Atomics.wait`; the page cannot block, so it spins until the answer comes.
Where the origin private file system is unavailable (some private windows),
files.js answers from memory and nothing survives a reload.

- SQLite's WASI build has no WAL and by default locks a database with a
  `NAME.lock` directory, which a reload mid-transaction leaves behind, so
  every later open would be busy. `sqlite_schema::open`, which opens both
  databases, uses the `unix-none` VFS on WASI instead: the Web Lock below
  keeps one owner, and each database has one connection. A refs database
  that does not open is an error the worker shows and retries, never a
  silent in-memory one.
- Every context counts the WASI monotonic clock (and `be_client::Clock::real`)
  from `performance.timeOrigin`, so an `Instant` the page makes means the
  same time in the be worker.
- An unload cannot wait for the be worker, so durability does not depend on
  it: a replica seals within `SEAL_INTERVAL` of an edit, and until then
  `be::status().unsaved` counts it, from the command the page sends to the
  module's next report (`Replicas::counts`). `beforeunload` asks first while
  anything is unsaved, and the worker seals while the browser asks;
  `be::flush` returns what was unsaved when it cannot wait (web) and what is
  left after waiting (native).

- The block stack (`be::start`) runs in a worker of its own, `web/be-worker.js`, as it
  runs on a thread natively: a second instance of the module on the page's
  shared memory, started through the wasm-bindgen glue (with its bare imports
  rewritten, since a worker has no import map) so that its sockets, timers,
  fetches and type-module workers are its own. It lives as long as the page.
  Its channels, `Shared` and `Condvar` are the native ones. `be::stop`
  cannot wait for it, so a stack started after another (switching
  organization and back) waits in its own worker until the earlier one's
  `serve` has returned (`be/web.rs`). The page's thread
  may never wait on a lock (a browser traps `memory.atomic.wait` there), so a
  lock the page shares with that worker is `wasi_threads::Mutex`, which spins
  on wasm, and the beui waker takes no lock.
- One tab owns the files: the page takes the `block-files` Web Lock before it
  starts the module, and a second tab says it is open in another tab and waits
  for the lock.
- `web/sw.js`, a service worker, keeps everything the page fetched from its
  origin (network first, apart from `/api` and `/auth`; a deployed build's
  immutable `/data/` files cache first, and a network silent for 3 s gets the
  kept answer), so a page opened once opens again without a network. The deployment uploads it as `/sw.js`
  (guides/hosting.md).
- `web-dev --fresh` deletes the browser profile, and the files with it.
- No browser test runs in verify, so check offline typing by hand: open a
  text block, `kill -STOP` the instance's be-server (a silent server; killing
  it is a refused one) or call Playwright's `context.setOffline(true)`, click
  the editor (`drive click` or `page.mouse.click`) and type with
  `page.keyboard.type`, then `page.reload()` with a `dialog`
  handler that accepts the `beforeunload` prompt a second later, and read
  the editor's value from `drive tree`. Compare `performance.timeOrigin`
  before and after to be sure the page did reload. `kill -CONT` the server
  before stopping web-dev, whose stop waits for it.
- The app closes its tab when it exits, except when the page is already
  leaving (a reload), since closing it would stop the reload.

## Hosted web build

The deployed bundle lives at `https://be3.pfg.pw/data/BUILD/web/index.html`; the
`/web` entry point selects the build that matches the deployed server, loading
versioned assets while keeping the address at `/web` so reloads update. Add account offers the default server from the deployed `build-info.json`; ordinary local and CI builds omit that default. Remote sign-in opens the selected server’s `/auth` page. The web-dev launcher's explicit development-server
selection stays local. Bunny's headers, caching and gateway setup are described
in guides/hosting.md. Relative asset URLs resolve against the document base,
which points at the build directory; page navigation and fragment cleanup must
use the current page URL instead.

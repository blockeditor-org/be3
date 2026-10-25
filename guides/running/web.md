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
the page, except those of the headless window: the browser is the window here. Its own
commands take their place (`drive` with no command lists them):
- `drive shot FILE [TARGET]` saves a screenshot through Playwright, of TARGET only when given.
- `drive upload FILE TARGET` clicks TARGET and answers the file chooser that opens with FILE.
- `drive size W H` resizes the page, and `drive reload` reloads it.
- `drive eval EXPRESSION` evaluates JavaScript in the page and prints the result.

The tree's coordinates are the page's CSS pixels when the device pixel ratio is 1, as it is in
the launcher's browser.

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

- The block stack (`be::start`) runs in a worker of its own, `web/be-worker.js`, as it
  runs on a thread natively: a second instance of the module on the page's
  shared memory, started through the wasm-bindgen glue (with its bare imports
  rewritten, since a worker has no import map) so that its sockets, timers,
  fetches and type-module workers are its own. It lives as long as the page.
  Its channels, `Shared` and `Condvar` are the native ones. The page's thread
  may never wait on a lock (a browser traps `memory.atomic.wait` there), so a
  lock the page shares with that worker is `wasi_threads::Mutex`, which spins
  on wasm, and the beui waker takes no lock.
- One tab owns the files: the page takes the `block-files` Web Lock before it
  starts the module, and a second tab says it is open in another tab and waits
  for the lock.
- `web/sw.js`, a service worker, keeps everything the page fetched from its
  origin (network first, apart from `/api` and `/auth`), so a page opened once
  opens again without a network. The deployment uploads it as `/sw.js`
  (guides/hosting.md).
- `web-dev --fresh` deletes the browser profile, and the files with it.
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

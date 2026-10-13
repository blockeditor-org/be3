# Running the web app

For a change that only shows in the browser, serve the web build and open it in a headless
Chromium, then drive the page from the shell (anything else is checked natively, which is
quicker: guides/running/drive.md):

    ./scripts/buck run //crates/block-app:web-dev
    source ~/.cache/be3/web-dev/env

The command builds the web bundle with every plugin and starts be-server and Caddy in front
of it on http://127.0.0.1:8090. It then opens the page in a headless Chromium and returns
once the app is up. The page is signed in to `dev@localhost` (password `dev-password`) on
that be-server, with a workspace called Dev open, so there is no account or workspace to make
first. The server's data and the browser's profile live in `~/.cache/be3/web-dev` and survive
restarts, so running the command again reopens the same workspace, which is how to pick up a
rebuild.
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
- `dev-workspace`: sign in to the page's own server as `dev@localhost`, registering it
  unless it is registered already. Then open the last workspace, or the first one, or a new
  one called Dev.
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

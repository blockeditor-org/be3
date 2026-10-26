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
- `drive screen shot FILE [TARGET]` saves a screenshot through Playwright, of TARGET only when given.
- `drive screen upload FILE TARGET` clicks TARGET and answers the file chooser that opens with FILE.
- `drive size W H` resizes the page, and `drive reload` reloads it.
- `drive eval EXPRESSION` evaluates JavaScript in the page and prints the result.

The tree's coordinates are the page's CSS pixels when the device pixel ratio is 1, as it is in
the launcher's browser.

## Hosted web build

The deployed bundle lives at `https://be3.pfg.pw/data/BUILD/web/index.html`; the
`/web` entry point selects the build that matches the deployed server, loading
versioned assets while keeping the address at `/web` so reloads update. Add account offers the default server from the deployed `build-info.json`; ordinary local and CI builds omit that default. Remote sign-in opens the selected server’s `/auth` page. The web-dev launcher's explicit development-server
selection stays local. Bunny's headers, caching and gateway setup are described
in guides/hosting.md. Relative asset URLs resolve against the document base,
which points at the build directory; page navigation and fragment cleanup must
use the current page URL instead.

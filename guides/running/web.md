# Running the web app

For a change that only shows in the browser: serve the web build, open it in a headless
Chromium, and drive the page from the shell. Anything else is quicker to check natively
(`guides/running/drive.md`).

## Starting it

```
./scripts/buck run //crates/block-app:web-dev
source ~/.cache/be3/web-dev/env
```

It needs Node and Playwright's Chromium: `npm install -g playwright` and
`npx playwright install chromium` where they are missing.

The command:

1. builds the web bundle with every plugin;
2. starts be-server, and Caddy in front of it, on http://127.0.0.1:8090;
3. opens the page in a headless Chromium and returns once the app is up.

The page is signed in to `dev@localhost` (password `dev-password`) on that be-server, with a
workspace called Dev open. The server's data and the browser's profile live in
`~/.cache/be3/web-dev` and survive restarts, so running the command again reopens the same
workspace, which is how to pick up a rebuild.

| Option or variable | Effect |
|---|---|
| `-- --fresh` | deletes the data first |
| `-- --webgl` | starts the browser without WebGPU, so the app and its plugins draw with WebGL |
| `-- --stop` | stops the browser and the servers |
| `BLOCK_WEB_DEV_DIR` | moves the `~/.cache/be3/web-dev` directory |
| `BLOCK_WEB_DEV_PORT` | moves the page; be-server, the browser's debugging port and Caddy's admin port take the three ports after it, for running two at once |

### Logs

- Everything the page and its plugins' workers log to the console goes to
  `~/.cache/be3/web-dev/browser.log` as `INFO:CONSOLE` lines, among the browser's own output.
- be-server's goes to `server.log` in the same directory.

### Page parameters

The launcher opens `http://127.0.0.1:8090/?dev-workspace&automation`. Both parameters work on
any page serving the bundle:

- `dev-workspace`: sign in to the page's own server as `dev@localhost`, registering it unless it
  is registered already, then open the last workspace, or the first one, or a new one called Dev.
- `automation`: answer `drive`'s commands through the bundle's `automate` export.

## Driving the page

`env` defines `drive`, which runs one command against the page and exits. It answers the native
app's commands (`guides/running/drive.md`), run by the app itself in the page, except those of
the headless window: the browser is the window here. These take their place (`drive` with no
command lists them):

| Command | Effect |
|---|---|
| `drive shot FILE [TARGET]` | saves a screenshot through Playwright, of TARGET only when given |
| `drive upload FILE TARGET` | clicks TARGET and answers the file chooser that opens with FILE |
| `drive size W H` | resizes the page |
| `drive reload` | reloads it |
| `drive eval EXPRESSION` | evaluates JavaScript in the page and prints the result |

The tree's coordinates are the page's CSS pixels when the device pixel ratio is 1, as it is in
the launcher's browser.

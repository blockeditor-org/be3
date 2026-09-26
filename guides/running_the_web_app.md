# Running the web app

To see a change working in the browser, serve the web build and open it in a headless
Chromium, then drive the page from the shell:

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

What the launcher opens is `http://127.0.0.1:8090/?dev-workspace&accessibility-tree`; those
two parameters work on any page serving the bundle:
- `dev-workspace`: sign in to the page's own server as `dev@localhost`, registering it
  unless it is registered already. Then open the last workspace, or the first one, or a new
  one called Dev.
- `accessibility-tree`: keep the accessibility tree as text for `drive tree` (see below).

## Seeing what is on screen

`drive tree` prints the accessibility tree, one node per line, indented under its parent,
in the same format as the native app's:

    Dialog "Add block" at 320,108 size 640x585
      ScrollView at 340,200 size 600x420
        Button at 620,226 size 132x124
          Label value="Text" at 673,325 size 26x15
        Button at 340,524 size 132x124 partly offscreen

Coordinates are CSS pixels from the page's top left corner, the same ones `drive` takes,
so the centre of a node is `x + width / 2, y + height / 2`. `offscreen` marks a node scrolled
out of view, and `focused` the node with keyboard focus.

As natively, the tree holds only what the host draws itself. The workspace's panes, its file
tree and every block's editor are drawn by plugins, and for those you take a screenshot and
read it:

    drive shot shot.png

`drive shot shot.png X Y W H` keeps only the region at X,Y of W by H, which is cheaper to
read than the whole page.

## Input

    drive click 455 220
    drive type hello
    drive key Control+z

- `drive click X Y right` right-clicks, and `drive dblclick X Y` double-clicks.
- `drive wheel X Y DY` scrolls at X,Y, down when DY is positive.
- `drive drag X1 Y1 X2 Y2` drags with the left button.
- `drive key` takes Playwright's key names, such as `Enter`, `Escape`, `ArrowDown` or
  `Shift+Tab`.
- Click a text field before typing into it.
- `drive upload FILE X Y` clicks X,Y and answers the file chooser that opens with FILE, which
  is how to add an image, a PDF or an audio file.
- `drive eval EXPRESSION` evaluates JavaScript in the page and prints the result.
- `drive reload` reloads the page, and `drive size W H` resizes it.
- `drive` with no command lists them all.

The app draws when something changes, so give it a moment after an input before reading the
tree or taking a screenshot.

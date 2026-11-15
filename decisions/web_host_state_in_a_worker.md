# The web's host state is kept by a worker; the page holds the one copy it reads

On the web, a worker of the host's own (`web/be-worker.js`, the worker that runs the block
stacks) opens the host's files from start-up, before and beside any stack: `host.sqlite3`
(saved accounts, settings, identities, input and display settings) and the refs database's
cache of the organizations each account last listed, which the Accounts and Organizations
screens show while no stack runs. It hands the page what they hold once, and keeps each
change the page sends after. The page reads its copy (`HostStateStore`,
`host_files::WorkspaceCache`) without a file, and changes it at once as it sends the change.
The page waits for that first answer without blocking its thread: `run_web` awaits it while
the page's start-up look stays up, then shows the first screen.

A native host takes the same copy and the same writes, through a keeper of its own in
process (`HostStateStore::open`), with no worker in between: it opens its files as it starts
and reads them at once.

## Why

- The page must make no file call (plans/web_page_never_waits.md): a file call on the page
  spins until `web/files.js` answers.
- One copy the host reads and one file it is kept in: there is one way to read and change the
  host state, the same on every host, and the native host tests cover it.
- The first screen depends on the saved accounts, so the page needs them before it shows one:
  an asynchronous wait of a few milliseconds behind the look the page already shows while it
  loads costs no visible step.
- The push receiver's state (`keep_push_keys`, `first_showing`) is not copied: another process
  writes it too (Android's receiver), so it is read and written in the file, only where the
  device decrypts pushes itself.

## Rejected

- **A copy in the browser's local storage, read by the page at start.** Two stores of the
  accounts and settings to keep in step, one of which could be left behind by any write
  (another tab, a crash between the two), against there being one obvious way to read them.
- **The page showing a first screen before the state arrives and switching after.** It shows
  the wrong screen (Accounts) to a person who is signed in, for a moment.

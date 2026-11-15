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

## Consequences

- A change to the host state reaches its file a moment after the page makes it: the page's
  copy changes at once, and the worker keeps the change when it next yields or finishes a
  pass, whichever comes first (`be_host_stack::run_first` keeps what was sent at the start of
  each pass), so it waits at most for the synchronous stretch it arrived in. A change made in
  the moment before the page unloads, within that stretch, is lost; a native host writes in
  place and loses none. The changes are sign-ins, account switches and settings, which the
  person makes and sees, and can make again.
- A change the worker cannot keep (out of space, a failed write), from the first one on, is
  told to the page by letter and shown as a host error that says what is lost: "This device
  could not save signing in to … : it will be gone after the page reloads (…)" (`Write::what`).
- The page reads its copy, not the file: a change another tab makes to `host.sqlite3` is not
  seen until the page starts again, as before (one tab owns the files, `block-files` lock).

## Rejected

- **A copy in the browser's local storage, read by the page at start.** Two stores of the
  accounts and settings to keep in step, one of which could be left behind by any write
  (another tab, a crash between the two), against there being one obvious way to read them.
- **The page showing a first screen before the state arrives and switching after.** It shows
  the wrong screen (Accounts) to a person who is signed in, for a moment.

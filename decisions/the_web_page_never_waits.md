# The web page never waits, and shares no memory with a thread

Nothing in be-host spins on a lock. A worker may wait for real (`Atomics.wait`), but a browser
traps `memory.atomic.wait` on a page's thread, so be-host's page never waits at all: it is
built so that nothing it needs is ever held by another thread.

The page shares its memory with no thread. The host's worker (`web/be-worker.js`, which runs
the block stacks and keeps the host's files) is an instance of the module with a memory of its
own. The page starts each stack there from a name and bytes (`be_host_stack::start_in_worker`),
and the two talk over a `MessagePort` only: commands, nudges and how much the page took one way;
the `Seen`'s updates, with the change notices, the replies' answers and the worker's notices,
the other; each wakes the other (guides/blocks/host.md, "What the page reads"). The page keeps its own copy of the
`Seen`; the worker keeps its own `Shared` and offline choices, and installs the app list the
page read and sent with the stack's start (`discovery::adopt`). A worker's letter to a page
mailbox travels as a copy of its bytes (`Post::across`).

The page binds no files channel, so a file call on it fails loudly (`web/wasi.js`,
`refuseFiles`), and it starts no thread, which would share its memory (`web/threads.js`,
`refuseThreads`; both in `own-memory-web-host-test`). So every lock the page takes is std's and
one no other thread takes: std's `Mutex` would trap there if it ever found one held, and never
does.

## Why

- A page that waits on a lock either traps or spins, and a spin on the page freezes the tab
  while another thread holds the lock.
- Messages over a memory of one's own are the shape plans/host_as_platform_adapter.md gives the
  block stack on every platform, so the web is not a special case later.

## Rejected

- A spinning mutex on the page (`be_wasm_threads::Mutex`): it waited without trapping, but
  still blocked the page's thread for as long as a worker held the lock.
- One shared memory with the page reading the worker's state synchronously: every such read is
  a lock the page might find held.

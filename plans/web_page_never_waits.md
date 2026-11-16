# The web page never waits

Depends on: none.

The owner's rule is that nothing spins on a lock. A worker waits for real (`Atomics.wait`), but
a browser traps `memory.atomic.wait` on a page's thread, so the page must never wait at all:
it has to be built so that nothing it needs is ever held by another thread.

be-host on the web is mostly built that way now (guides/running/web.md, "Files and offline").
The host's worker (`web/be-worker.js`, which runs the block stacks and keeps the host's files)
is an instance of the module with a memory of its own. The page starts each work there from a
name and bytes (`be_host_stack::start_in_worker`), and the two talk over a `MessagePort`
only: commands, nudges and how much the page took one way; the `Seen`'s updates, with the
change notices, the replies' answers and the worker's notices, and wakes the other
(guides/blocks/host.md, "What the page reads"). The page keeps its own copy of the `Seen` and
reads the block types the modules know from it (`Seen::kinds`); the worker keeps its own
`Shared`, offline choices and app list. A worker's letter to a page mailbox travels as a copy of
its bytes (`Post::across`). The page binds no files channel, so a file call on it fails loudly
(`web/wasi.js`, `refuseFiles`), and starts no thread, which would share its memory
(`web/threads.js`, `refuseThreads`; both in `own-memory-web-host-test`). So every lock the page
takes is std's and one no other thread takes, and nothing on the page spins: std's `Mutex`
would trap there if it ever found one held, and never does.

## What is left

1. Make updates smaller. An update sends whole what is not a persistent map, and a map's
   changed entry whole: an open block's `Content` on each of its edits, the outbox and the
   owing lists on each change, and an editor's queued frames. Send content as its edit (the
   block's own operations or a byte delta), or keep a value's `Arc` identity across updates so
   one the worker did not replace is never sent again.
2. Fetch the app list once per start. The page and the worker each fetch it
   (`discovery::load`), since neither can read the other's: the worker could send the page what
   it read, or the page the worker.

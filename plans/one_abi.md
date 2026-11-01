# One calling convention for guests

Not started. The question was "why can't everything be local, and then define most
functions as async?" The answer this plan reaches: almost everything already is. A
guest's link to its host is two wasm imports answered on its own thread
(`host_send`, `host_receive`), and every async func, stream and resource already
travels through them as be-ops frames. What breaks the picture is the *direct call*:
a sync func gets a wasm import or export of its own, and its call is carried in a
second item format beside the frames, implemented once in Rust and twice in JS.
Turning sync funcs into frames too (a one-way `Start` for a sync import or a sync
export without a result, an ordinary op for a sync export with one) deletes that
format, its three copies, the per-world import and export lists and the
`accept_direct` half of every host, and leaves two ways a call travels, chosen by
who answers it: on the guest's thread (`local`, only the GPU) or anywhere else (an
op frame). Nothing on the GPU path changes.

The idea this plan started from, every import a `local`-style call into a
generated shim with flattened arguments, is turned down below ("Why not a shim per
import"): the shim would have to re-encode every type of every world in Rust for
wasmtime and again in JS or a second wasm module for the web, which adds code
rather than removing it.

## What each path does today

**Frames (async funcs, streams, resources).** The guest encodes a `be_ops::Frame`
with postcard (`crates/be-ops/src/lib.rs`, `Connection::start` and the rest), and
`block_guest_abi::wasm::send` hands it to the host with `host_send`
(`crates/block-guest-abi/src/wasm.rs`); the host's frames come back through
`host_receive` at the next `plugin_step`. Natively wasmtime answers both imports in
`crates/block-wasm-host` (`gpu::link_transport`, `transport.rs`); on the web the
worker's JS keeps an inbox and an outbox (`transport()` in
`crates/block-app/web/plugin.js`, `host` in the inline worker of
`crates/block-guest-host/src/web.rs`). The answer arrives later, in a frame, from
whoever holds the other `Connection`.

**Direct calls (sync funcs).** Each sync import is a wasm import of its own,
`fn(has_parent: i32, parent: i64, pointer: u32, length: u32)`, generated in
`export_<world>!()`'s registrar (`crates/be-ops-bindgen/src/lib.rs`, `import_shims`
and `import_arms`), with postcard arguments in guest memory. The guest first flushes
its queued frames (`be_ops::guest::flush`) and then calls the import; the runtime's
host function builds a `block_guest_abi::direct` call item (MARK `ff ff ff ff`, kind
`CALL`, flags, a u64 parent, a u16 name length, the name `module#func`, the args) and
puts it in the same outbox as the frames:

- natively `crates/block-wasm-host/src/direct.rs` `link`, one `func_wrap` per entry of
  `Host::with_direct(embed::IMPORTS)`;
- on the web `forward` and `callItem` in `plugin.js` and again in `web.rs`'s worker,
  given the list `embed::IMPORTS` in the worker's `start` message
  (`crates/block-app-plugin-host/src/web/worker.rs`, `WebGuest::start`'s `imports`).

The guest never gets an answer: the generator refuses a sync import with a result,
because on the web the host's state is on another thread (guides/type_modules.md,
"The rule").

Each sync export is a wasm export of its own (`shims` in the generator, which call
`be_ops::guest::export`), plus `plugin_alloc`. The host's `Connection::call_direct`
or `notify_direct` puts a call item in its outbound; the runtime spots it among the
frames (`Plugin::deliver` in block-wasm-host, `deliver` in `plugin.js`, the `step`
handler in `web.rs`), steps the guest through the frames before it, allocates room
with `plugin_alloc`, copies the args in, calls the export, reads the packed
`(pointer, length)` it returns and, if the call was `answered`, posts a `RETURNED`
item. The host's `Connection::receive` matches returned items to its direct calls
in order (`direct: VecDeque<OpId>`) and turns each into `Incoming::Completed`; a guest
import arrives as `Incoming::Called`, which hosts match with the generated
`Call::accept_direct` beside `Call::accept`. In process (tests, `host-types`, the
type modules' `Sim`) there are no wasm imports: `guest::direct` falls back to
`Connection::notify_direct`, and the guest executor turns `Incoming::Called` into the
same `dispatch` call an op gets, with op 0 (`Inner::call_direct` in
`crates/be-ops/src/guest.rs`). So a sync export is already dispatched as an op whose
answer is immediate; only the transport differs.

**Local funcs (`be3:gpu/device` only).** `crates/be-ops-bindgen/src/local.rs`. Every
func, constructor, method and drop is a wasm import with flattened arguments that
returns its result into the guest at once: handles and scalars as wasm values, lists
of scalars as pointer and length, anything else postcard written into room the guest
offers, with `[returned]` fetching what did not fit. Natively wasmtime host functions
answer it (`gpu::link`, block-gpu-host's `Gpu`); on the web block-gpu-shim, a second
wasm module in the plugin's worker, exports each as `module#name` and `plugin.js`
binds the plugin's imports to those exports. Either way the answer is computed on
the guest's own thread, which is what makes a synchronous result possible.

**Network.** `network: true` (be-protocol's be3:server). The same frames over a
socket through `be_ops::net::Peer`; the client is a `Send` caller rather than the
guest executor. Sync funcs are refused, since a direct call needs a wasm guest's
thread.

## What has to differ, and what doesn't

| | answered by | on which thread | does the caller get a value back synchronously | encoding |
|---|---|---|---|---|
| local | a host on the guest's thread (wasmtime, block-gpu-shim) | the guest's | yes | flattened wasm values |
| op (guest ⇄ host) | the host | the host's (main thread on the web) | no: later, in a frame | postcard frame |
| op (network) | the server | another machine | no | postcard frame |
| direct import | the host | the host's | no, never | postcard in a call item |
| direct export | the guest | the guest's | the host awaits it like an op | postcard in a call item |

The only line that is truly different is the first: a synchronous answer needs the
answerer on the caller's thread, and the GPU needs it (a texture create returns a
handle the next call uses, and a draw must cost a few wasm values). Everything else
is answered on another thread, so it can only be answered later, which is exactly an
op. A direct import differs from a one-way op only in its envelope, and a direct
export from an op only in its envelope and in being run at delivery rather than at
the next step. The envelope is the cost: a second format, a list of names each
runtime must be handed, a wasm import or export per function, and the ordering rule
that frames must be flushed before a call and stepped before an export.

What doesn't have to differ: ordering (one outbound queue in the `Connection` keeps
everything in order without flushing), cancellation and parents (a one-way frame
carries a parent like any `Start`), the dispatch on the guest (already shared), and
the host's matching (`accept` already decodes a `Started` by method id).

## Proposed model

Two ways a call travels, chosen by who answers it:

- **Local**: unchanged. Only interfaces whose host runs on the guest's thread on
  every platform (`local: [...]`), only the GPU today.
- **Op**: everything else, as a be-ops frame through `host_send`/`host_receive`,
  natively, on the web, in process and over the network alike.

Within ops, the WIT decides the shape:

- **`async func`**: an op, as today. The guest gets a `Pending<T>` at once.
- **Sync import** (no result, as the generator already requires): a *one-way*
  `Start` with op id 0. The kernel ids start at 1 with the side's bit, so 0 is free.
  The receiving `Connection` does not register it as served and surfaces it as
  `Incoming::Started` like any op; `embed::Call::accept` builds its case, which for a
  sync func has no `Reply`, so nothing can answer it, and `complete(0, ..)` is a
  no-op. A one-way start whose parent has ended is dropped (an op's would be answered
  `Canceled`, and there is no one to answer). Guest code is unchanged:
  `guest::shell::set_title(..)` still returns `()` at once.
- **Sync export with a result**: an ordinary op the host starts
  (`embed::exports::...` returns a `Ticket<T>` as an async export's does). The guest's
  `dispatch` answers `Serve::Done` in the same `Executor::step` that read the
  `Start`, so the `Complete` follows the frames the export made, as the
  `RETURNED` item does today. Every runtime already steps the guest right after
  delivering (`Wasm::step` in `crates/block-app-plugin-host/src/wasm.rs`, the
  workers' `step` messages), so the answer comes back in the same batch.
- **Sync export without a result**: a one-way `Start`, as `notify_direct` is today.
- **Streams**: unchanged; they are ops.
- **Resources**: unchanged. A sync method is a one-way `Start` with the resource's op
  as parent, as its direct call names the resource as parent today; `resource_drop`
  stays `Cancel`.
- **The GPU's synchronous creates**: unchanged, local. The plan touches nothing in
  `local.rs`, block-gpu-host or block-gpu-shim.
- **Network**: unchanged. One-way frames would also work over a socket, so the
  generator's refusal of sync funcs in a network world could be lifted, but nothing
  needs that, and an `async func` returning nothing is the one way to do it there
  today; leave it refused.

The guest's whole wasm interface becomes `block_guest_abi`'s: imports
`be3_host.host_send/host_receive/host_wake/host_panic`, the local interfaces' imports,
and exports `plugin_start/plugin_step/plugin_shutdown`. `export_<world>!()`,
`plugin_alloc`, `embed::IMPORTS`, `Host::with_direct` and the worker's `imports`
message all go, since there is nothing per function left to link.

### Why not a shim per import

The starting idea was every import as a local-style call with flattened arguments,
returning an op handle for an async func, and a generated shim that queues the call.
Tested against the code:

- The shim would encode the frame. Today the guest encodes it, with the types it
  already has, in code generated once. A shim per import would need every type of
  every world on the shim's side: generated Rust linked into block-wasm-host (and
  every runtime that hosts a guest: block-app-plugin-host, be-types-host, the
  in-process hosts) and, on the web, JS that writes postcard, or a wasm shim like
  block-gpu-shim compiled against every world. That is a second encoder of
  everything, in two languages, to save the guest a `Vec` copy.
- Flattening wins on the GPU because the answer is computed on the same thread from
  the same values; there is no encoding at all. For a call answered on another
  thread the values have to be serialized anyway, so flattening only moves the
  serializer.
- It would keep a wasm import per function and the list of them each runtime must be
  handed, which is most of what this plan deletes.

So the useful part of the idea, one convention and postcard only where something
crosses a thread or a socket, is kept, with the guest rather than a shim doing the
encoding, and one pair of imports rather than one per function.

### Why not synchronous results from the host

"Everything local" could also mean the host answering a sync import with a value
while the guest waits. Natively the guest could take the host's lock from a wasmtime
host function; on the web the worker could `Atomics.wait` on a SharedArrayBuffer
cell that the main thread fills (the main thread itself may not block, but it need
not). Turned down:

- A host waiting on a sync export while the guest waits on a sync import deadlocks,
  and a slow host frame would stall every plugin waiting on it.
- Type modules must step deterministically (block-type-guest's `Sim`); a blocking
  import makes a step depend on when the host answered.
- It cannot work over the network, so it would keep a second convention.
- An `async func` already gives a guest its value one step later, with cancellation.

So "define most functions as async" is the right half of the question: anything that
needs a value from the host is an `async func`, as the generator already requires.

## The web

No blocking, no new SharedArrayBuffer or Atomics. The worker's job shrinks to: put the
host's frames in the inbox, call `plugin_step`, post the outbox. `plugin.js` loses
`MARK`, `marked`, `callItem`, `forward`, `callExport`, the `direct` list and the
call-item branch of `deliver`; `web.rs`'s inline worker loses the same, and its
`imports` resolver answers only `be3_host`. The two workers then differ only in what
the plugin worker adds (the GPU shim, WASI, threads, dynamic linking), which is what
`wasm-runner` is merging; whichever lands second has less to merge. If wasm-runner
lands first, step 3 below deletes the one remaining copy; if this plan's step 3 lands
first, wasm-runner merges two workers that already agree.

Threaded plugins keep their shared memory and wasi-threads as now; this plan does not
touch them.

## Performance

The GPU hot path does not change: local calls keep their flattened imports, and
nothing on that path reads the transport. `//crates/block-wasm-host:example-gpu-calls`
is the guard; there is no `be3.release` flag, a release build is the `-m release`
modifier (buck/constraints/BUCK, buck/platforms/modifiers.bzl), as
guides/performance.md already says:

    ./scripts/buck run -m release //crates/block-wasm-host:example-gpu-calls

Built that way on the build server in November 2026 (about two minutes, 98% cached),
it printed `100000 x (set pipeline, set viewport, set scissor, draw), 20 steps: fastest
129.8 ns per call, mean 325.1 ns per call`, the fastest matching the guide's 130 ns; the
mean is noisy on a shared VM, so compare fastest against fastest. Each step below should
leave the fastest within noise of that, which it should do trivially since none of them
touches the code it times.

Sync calls cost about the same either way. A sync import today is a flush (one
`host_send` per queued frame) plus one wasm import call that copies the args into an
item with a ~40-byte name; as a frame it is a postcard `Start` with an 8-byte method
id, queued and sent with the step's other frames by the same `host_send` per frame. A
sync export today is `plugin_alloc`, a copy in, the call and a copy out; as a frame it
is one copy in through `host_receive` and the executor's `dispatch`, which both paths
already run. Host-side matching goes from a name hash per call (`Called::method`) to
the id in the frame. Nothing measured yet says sync calls are hot (they are
per-input-event or per-frame, a few per frame, against hundreds of GPU calls), so no
benchmark is added; if one were wanted, `host_send` taking a batch of frames is the
obvious next saving, for frames and one-way calls alike.

## Steps

Each lands alone, with `./scripts/verify` green, and leaves the guide describing
what is there.

1. **One-way frames in the kernel.** `Connection::notify(parent, method, args)` sends
   `Start { op: 0, .. }`; `receive` surfaces it as `Incoming::Started` without
   registering it, and drops it if its parent has ended. Bindgen's `accept` handles a
   sync func's case from a `Started` (it has the arms already, in `accept_direct`).
   Hosts switch from `Incoming::Called` + `accept_direct` to the `Started` they
   already match (block-app-plugin-host `wire.rs`, be-types-host `modules.rs`,
   block-ui-test `beui.rs` and `store.rs`, block-plugin-api `requested.rs`,
   block-editor-plugin's font test). Still sent as direct items.
   Deletes: `accept_direct` in both generated places, the `Called` matching in each
   host (~120 lines).
2. **Sync imports as frames.** The generated `guest::<import>` sync func calls
   `be_ops::guest::notify(method, &args)`, which queues on the connection in process
   and on wasm alike. Deletes: `guest::direct`, `flush`, `Imports`,
   `register_imports`, the registrar's `import_shims` and `import_arms`,
   `embed::IMPORTS`, `Host::with_direct` and block-wasm-host's `direct::link`, the
   worker `imports` message and the `forward`/`callItem` halves of both JS workers,
   `Connection::notify_direct`, `Incoming::Called` and `Called` (~250 lines).
3. **Sync exports as ops.** `embed::exports` sync funcs `start` an op (with a result)
   or `notify` (without); the guest needs nothing new, since `dispatch` already serves
   them. Deletes: the generated export `shims`, `plugin_alloc`, `export_<world>!()`
   and its two call sites, `be_ops::guest::export`/`alloc`/`RESULT`/`call_direct`/
   `returning`, `Connection::call_direct`/`return_direct` and the `direct` queue,
   block-wasm-host's `direct::call` and the call-item branch of `Plugin::deliver`,
   `callExport` and the call-item branches of both JS workers, and
   `block_guest_abi::direct` itself (~330 lines).
4. **Guide.** Rewrite guides/type_modules.md's "Operations and WIT" call-path
   paragraphs (direct items, "The rule", the closing three-ways summary) to the two
   ways above, and drop `export_<world>!` from guides/adding_a_plugin_editor.md and
   plugin_libraries.md where they name it.

Steps 2 and 3 can land in either order; together they remove the last call item, so
`block_guest_abi::direct` goes with whichever is second.

## Risks and open questions

- **Order of a sync export against frames.** Today the runtime steps the guest
  through earlier frames, then runs the export at delivery. As a frame it is read in
  order by the same step, which handles one item completely before the next, so the
  order is the same; the change is that the export runs inside `plugin_step` rather
  than beside it. A test that relies on reading an export's answer after `deliver`
  without a `step` would need the step; every runtime already steps.
- **A sync export's `dispatch` must answer at once.** A sync method's arm already
  returns `Serve::Done`; a `Later` from one is a generator bug, answered
  `Unsupported` today and still.
- **op 0 as "one-way".** It is a convention inside be-ops, not a frozen format (the
  plugin protocol is stable only within one source tree), and hosts never see op 0
  except on a `Started` whose case cannot be answered. Alternatively a
  `Frame::Notify` variant; op 0 keeps one `Started` path through every host, which is
  the point.
- **A plugin built before the change** stops loading against a host after it, and the
  other way round. Plugins are built from the same tree as their host, so this only
  matters to a stale build cache.
- **Open: a GPU over frames.** wgpu's own web backend allocates ids on the client and
  never waits for a create; the GPU world could do the same with one-way frames
  batched per step, and its reads (`map_async`, `take-error`) as ops. That would make
  the GPU's calls one-way too and might beat 130 ns a call when batched, but it moves
  validation errors away from the call that caused them and is a large change to
  block-gpu-guest. Not part of this plan; measure before considering it.

## Size

Removed, about 700 lines: `block_guest_abi::direct` (81), block-wasm-host's `direct.rs`
and its uses (~100), the call-item code in `plugin.js` (~75) and `web.rs`'s worker
(~80, plus the `imports` plumbing), be-ops' direct paths in `lib.rs` and `guest.rs`
(~140), the generator's import and export shims, `IMPORTS`, `export_<world>!` and
`accept_direct` (~170), and the hosts' `Called` branches (~60). Added, about 80:
`notify` and op-0 handling in be-ops (~30), the generator's switch to `start`/`notify`
(~30), and host matches (~20). Net about -600, half of it in places that had to agree
byte for byte across Rust and JS.

Worth doing: it removes a whole wire format and the only JS that had to track a Rust
byte layout, makes in-process and wasm guests take one path for sync calls, and costs
nothing on the GPU path. It is not urgent; nothing is broken by the direct items.

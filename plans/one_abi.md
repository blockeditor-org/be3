# One calling convention for guests

Not started. The question was "why can't everything be local, and then define most
functions as async?", followed by "shouldn't a call with no result that is answered
later be marked async in the WIT?". This plan answers both with one rule:

**A `func` in WIT is answered synchronously, on the caller's thread. Anything else is
an `async func`.** Only `local` interfaces (the GPU today) answer on the caller's
thread, so they are the only place a sync func is allowed; the generator refuses one
anywhere else, import or export, as it already does for a network world. Every other
call is a be-ops op: a postcard frame through `host_send`/`host_receive`, answered
later with a `Complete`.

Today about ninety sync funcs break that rule. A sync import with no result looks
synchronous to the guest but is queued and handled on the host's thread later. A sync
export looks synchronous to the host, but the host has to wait for it like an op.
Both travel in a second item format beside the frames, the `direct` call items,
implemented once in Rust and twice in JS. Marking them `async` in the WIT puts them on
the op path, which deletes:

- the `direct` format and its JS copies;
- each sync func's own wasm import and export, `export_<world>!`, `plugin_alloc`;
- the `IMPORTS` list each runtime must be handed;
- `accept_direct` and `Incoming::Called` in every host;
- the generator's `Shape::Direct`.

Nothing on the GPU path changes.

## What each path does today

**Frames (async funcs, streams, resources).** The guest encodes a `be_ops::Frame`
with postcard (`crates/be-ops/src/lib.rs`, `Connection::start` and the rest), and
`block_guest_abi::wasm::send` hands it to the host with `host_send`
(`crates/block-guest-abi/src/wasm.rs`); the host's frames come back through
`host_receive` at the next `plugin_step`. Natively wasmtime answers both imports in
`crates/block-wasm-host` (`gpu::link_transport`); on the web the worker's JS keeps an
inbox and an outbox (`transport()` in `crates/block-app/web/plugin.js`, `host` in the
inline worker of `crates/block-guest-host/src/web.rs`). The answer arrives later, in a
frame, from whoever holds the other `Connection`.

**Direct calls (sync funcs).** Each sync import is a wasm import of its own,
`fn(has_parent: i32, parent: i64, pointer: u32, length: u32)`, generated in
`export_<world>!()`'s registrar (`crates/be-ops-bindgen/src/lib.rs`, `import_shims`
and `import_arms`), with postcard arguments in guest memory. The guest first flushes
its queued frames (`be_ops::guest::flush`) and then calls the import. The runtime's
host function builds a `block_guest_abi::direct` call item (MARK `ff ff ff ff`, kind
`CALL`, flags, a u64 parent, a u16 name length, the name `module#func`, the args) and
puts it in the same outbox as the frames. There are three copies of that code:

- natively `crates/block-wasm-host/src/direct.rs` `link`, one `func_wrap` per entry of
  `Host::with_direct(embed::IMPORTS)`;
- on the web `forward` and `callItem` in `plugin.js`, and again in `web.rs`'s worker,
  each handed the list `embed::IMPORTS` in the worker's `start` message
  (`crates/block-app-plugin-host/src/web/worker.rs`, `WebGuest::start`'s `imports`).

The guest never gets an answer: the generator refuses a sync import with a result,
because on the web the host's state is on another thread (guides/type_modules.md,
"The rule"). The call is asynchronous in all but name.

Each sync export is a wasm export of its own (`shims` in the generator, which call
`be_ops::guest::export`), plus `plugin_alloc`. Here is what happens to one:

1. The host's `Connection::call_direct` or `notify_direct` puts a call item in its
   outbound.
2. The runtime spots the item among the frames (`Plugin::deliver` in block-wasm-host,
   `deliver` in `plugin.js`, the `step` handler in `web.rs`).
3. It steps the guest through the frames before the item, allocates room with
   `plugin_alloc`, copies the args in and calls the export.
4. It reads the packed `(pointer, length)` the export returns and, if the call was
   `answered`, posts a `RETURNED` item.
5. The host's `Connection::receive` matches returned items to its direct calls in
   order (`direct: VecDeque<OpId>`) and turns each into `Incoming::Completed`.

So the host already treats a sync export as an op whose answer comes later. A guest
import arrives as `Incoming::Called`, which hosts match with the generated
`Call::accept_direct` beside `Call::accept`.

In process (tests, `host-types`, the type modules' `Sim`) there are no wasm imports.
`guest::direct` falls back to `Connection::notify_direct`, and the guest executor
turns `Incoming::Called` into the same `dispatch` call an op gets, with op 0
(`Inner::call_direct` in `crates/be-ops/src/guest.rs`).

**Local funcs (`be3:gpu/device` only).** `crates/be-ops-bindgen/src/local.rs`. Every
func, constructor, method and drop is a wasm import with flattened arguments, and it
returns its result into the guest at once:

- handles and scalars go as wasm values;
- lists of scalars go as a pointer and a length;
- anything else is postcard written into room the guest offers, with `[returned]`
  fetching what did not fit.

Natively wasmtime host functions answer it (`gpu::link`, block-gpu-host's `Gpu`). On
the web block-gpu-shim answers it: a second wasm module in the plugin's worker, which
exports each import as `module#name`, and `plugin.js` binds the plugin's imports to
those exports. Either way the answer is computed on the guest's own thread, so these
are the only calls that are really synchronous.

**Network.** `network: true` (be-protocol's be3:server). The same frames over a
socket through `be_ops::net::Peer`; the client is a `Send` caller rather than the
guest executor. Sync funcs are already refused there, so this world already follows
the rule.

## What has to differ, and what doesn't

| | answered by | on which thread | the caller has the answer | encoding |
|---|---|---|---|---|
| local | a host on the guest's thread (wasmtime, block-gpu-shim) | the guest's | when the call returns | flattened wasm values |
| op (guest ⇄ host) | the host, or the guest for an export | the other side's | later, in a frame | postcard frame |
| op (network) | the server | another machine | later | postcard frame |
| sync import today | the host | the host's | never | postcard in a call item |
| sync export today | the guest | the guest's | later: the host waits as for an op | postcard in a call item |

Only the first line is truly different. A synchronous answer needs the answerer on
the caller's thread, and the GPU needs one: a texture create returns a handle the next
call uses, and a draw must cost a few wasm values. Everything else is answered on
another thread, so it is an op, and should say so in the WIT. The last two lines are
ops in a different envelope. That envelope costs:

- a second format;
- a list of names each runtime must be handed;
- a wasm import or export per function;
- an ordering rule: flush the frames before a call, and step through them before an
  export.

What doesn't have to differ: ordering (one outbound queue per `Connection`),
cancellation and parents, the dispatch on the guest (already shared), and the host's
matching (`accept` already decodes a `Started` by method id).

## Proposed model

Two ways a call travels, chosen by who answers it, and the WIT says which:

- **`func`**: only in a `local` interface; a wasm import that returns its answer.
  Unchanged.
- **`async func`, `stream<T>`, resources**: ops, everywhere else, natively, on the
  web, in process and over the network alike. Unchanged.

The generator refuses a `func` in any interface that is not `local`, with the message
the network world gives today ("make it an async func"). `Shape::Direct` goes.

### Fire and forget

Most of today's sync imports are notifications (`shell.copy-text`,
`host.frame-needed`, `blocks.write-content`). As `async func`s returning nothing they
come back to the guest as `Pending<()>`. Dropping a `Pending` cancels its op, so a
notification needs a way to be sent without being waited on and without being
canceled. The options:

- **A flag on the func, or a generated wrapper that hides the `Pending`.** This is
  hidden asynchrony again, under another name. Rejected.
- **Hold or await every `Pending`.** Await where the guest wants to know the host has
  acted. Elsewhere it is a pointless task per call. Rejected as the only way.
- **`Pending::detach(self)`.** The op stays alive without anyone waiting for it, and
  its outcome, an error included, is dropped when its `Complete` arrives. Chosen.

So a notification is `guest::shell::copy_text(instance, text).detach();`, and the
call site says that nothing waits for the answer. `Pending` is a future and so
`#[must_use]`, and `let _ = pending` trips clippy's `let_underscore_future`. Both are
errors under `-D warnings` (buck/toolchains/BUCK), so a call that is neither awaited,
held nor detached does not build. `detach` is about fifteen lines in
`crates/be-ops/src/guest.rs`: mark the call's entry in `calls` as detached, and have
`Complete` remove a detached entry rather than wake a waker. The network client's
`net::Pending` does not get one until something needs it.

On the host side, each of these imports' `embed` cases now carries a `Reply<()>`,
which the host answers once it has acted. That answer is the point of the change: a
guest that cares can await it, for example await `write-content` before it reports a
save. A host that drops a `Reply` unanswered leaves the guest's entry waiting until
the connection closes, as with any op today.

### Exports the guest finishes in the same step

Today's sync exports (the plugin's `editor` methods, `describe`, `screens`,
`configure`; the type module's `replica` methods and `start`, `blank`, `merge3`,
`describe`, `to-file`, `claims`, `apply`) become `async func`s. Their generated guest
trait methods return `impl Future<Output = T> + 'static`, as async exports' already
do. An implementation that answers at once does its work in the method and returns
`std::future::ready(value)`.

To keep that as cheap as a sync export is today, the generated `dispatch` polls the
future once before boxing it. If it is ready, the answer is `Serve::Done`, with no
task and no allocation. If not, it is `Serve::Later` as now. That is about fifteen
lines in the generator and executor.

An implementation that is ready at once answers in the same `Executor::step` that read
the `Start`, after the frames it made, as a `RETURNED` item does today. Every runtime
steps the guest right after delivering (`Wasm::step` in
`crates/block-app-plugin-host/src/wasm.rs`, the workers' `step` messages), so the
answer comes back in the same batch.

On the host, `embed::exports::...` already returns a `Ticket<T>` for an async export,
and does the same for these. For a former sync export with no result, the host keeps
or ignores the `Ticket`, as it does for an async export it does not wait on.

### Everything else

- **Streams**: unchanged; they are ops.
- **Resources**: unchanged. A resource's methods are now all async, so each is an op
  with the resource as parent, and `resource_drop` stays `Cancel`.
- **The GPU's synchronous creates**: unchanged and local. Nothing in `local.rs`,
  block-gpu-host or block-gpu-shim changes.
- **Network**: unchanged; it already has no sync funcs.

The guest's whole wasm interface becomes `block_guest_abi`'s:

- imports `be3_host.host_send/host_receive/host_wake/host_panic`, plus the local
  interfaces' imports;
- exports `plugin_start/plugin_step/plugin_shutdown`.

### Why not a shim per import

The first idea was every import as a local-style call with flattened arguments,
returning an op handle for an async func, and a generated shim that queues the call.
Tested against the code:

- The shim would encode the frame. Today the guest encodes it with the types it
  already has, in code generated once. A shim per import would need every type of every
  world on the shim's side: generated Rust linked into every runtime that hosts a guest
  and, on the web, JS that writes postcard or a wasm shim like block-gpu-shim compiled
  against every world. That is a second encoder of everything, in two languages.
- Flattening wins on the GPU because the answer is computed on the same thread from the
  same values. A call answered on another thread must be serialized anyway, so
  flattening only moves the serializer.
- It would keep a wasm import per function and the list of them each runtime must be
  handed, which is most of what this plan deletes.

### Why not synchronous answers from the host

A host could answer a sync import with a value while the guest waits. Natively the
guest could take the host's lock in a wasmtime host function. On the web the worker
could `Atomics.wait` on a SharedArrayBuffer cell that the main thread fills (the main
thread itself may not block, but it need not). Turned down:

- A host waiting on an export while the guest waits on an import deadlocks, and a slow
  host frame would stall every guest waiting on it.
- Type modules must step deterministically (block-type-guest's `Sim`), and a blocking
  import makes a step depend on when the host answered.
- It cannot work over the network.
- An `async func` already gives the guest its value one step later, with cancellation.

## What changes in the WIT

Every non-local `func` that does not return a stream becomes an `async func`. That is
91 funcs, by the file each is in.

Imports, 58, all returning nothing, so each guest call site gains `.detach()` or an
`.await`, and each host arm gains a `reply.send(..)`:

- `crates/block-plugin-api/wit/blocks.wit` (`blocks`), 7: `open-block`, `act`,
  `version`, `create-block`, `set-parent`, `define`, `write-content`.
- `host-services.wit`, 2: `release-revision`, `set-access`.
- `host.wit`, 3: `layout`, `frame-needed`, `missing-characters`.
- `shell.wit`, 28: `load`, `reload`, `focus-app`, `menu`, `run-bar-action`,
  `child-menu-pick`, `copy-text`, `copy-image`, `paste`, `drag-block`,
  `drag-accepted`, `aspect-ratio`, `intrinsic-size`, `act`, `leave-frame`,
  `view-stopped`, `present`, `change-view`, `play-audio`, `focused`,
  `handles-dismiss`, `close-dialog`, `dismiss-child`, `commit-child`, `picked`,
  `show-card`, `artifact-described`, `artifact-edited`.
- `deps/debug/debug.wit`, 2: `performance`, `simulate-access`.
- `deps/desktop/desktop.wit`, 8: `windows.request`, `power.request`,
  `media.request`, `wifi.request`, `epaper.refresh`, `notifications.report`,
  `programs.request`, `tray.request`.
- `crates/block-type-api/wit/host.wit`, 8: `claim-lease`, `report-clean`, `send`,
  `report-status`, `described`, `history`, `conflict`, `error`.

Exports, 33, where each guest implementation returns a future (`ready(..)` for the
same step) and each host caller gets a `Ticket`:

- `crates/block-plugin-api/wit/plugin.wit` (`plugin` and its `editor` resource), 19:
  `menu-pick`, `child-bar`, `child-focused`, `set-editable`, `view-changed`,
  `presenting-changed`, `presence`, `focus-changed`, `resized`, `drag-over`,
  `drag-left`, `artifact-settings`, `dismiss`, `show-dialog`, `describe`,
  `run-action`, `node-action`, `screens`, `configure`.
- `crates/block-type-api/wit/replicas.wit` (`replicas` and its `replica` resource),
  14: `reopen`, `message`, `attach`, `detach`, `head-moved`, `lease`, `claim`,
  `start`, `blank`, `merge3`, `describe`, `to-file`, `claims`, `apply`.

The list was taken from `rg ': func\('` over the `wit/` dirs, skipping the
stream-returning ones. Recount it when the step lands. The `replica` method `detach` and `Pending::detach` share a name but not a type: one is a host-side handle's method, the other the guest's.

Unchanged: `crates/be-ops/test-wit` and `crates/be-protocol/wit` (no sync funcs but
streams), `crates/block-gpu-abi/wit/device.wit` (local, 89 sync funcs).

The callers affected:

- **Guest call sites of the imports**: about 70, nearly all one wrapper each in the
  plugin side of the stack (block-editor-plugin, beui-platform-plugin, the
  `be3:desktop` users in workspace-ui and the shell editors) and in block-type-guest's
  host calls. The busiest names (`windows::request`, 13; `wifi::request`, 5) are
  desktop requests.
- **Host arms that answer them**:
  - block-app-plugin-host `wire.rs` (the `Called` branch merges into the `Started`
    one);
  - block-plugin-api `requested.rs` (each helper takes a `Started` and returns the
    request with its `Reply`);
  - be-types-host `modules.rs` and its tests;
  - block-ui-test `beui.rs` and `store.rs`;
  - block-editor-plugin's font test;
  - block-type-guest's `Sim`.
- **Guest implementations of the exports**: the plugin SDK's `plugin` and `editor`
  impls (block-editor-plugin) and block-type-guest's `replicas` impl, plus the fake
  guests in tests.
- **Host callers of the exports**: block-app-plugin-host's runtime and instances, and
  be-types-host. Their generated functions already return `Ticket`s for the async
  ones.

## The web

No blocking, and no new SharedArrayBuffer or Atomics. The worker's job shrinks to:
put the host's frames in the inbox, call `plugin_step`, post the outbox.

- `plugin.js` loses `MARK`, `marked`, `callItem`, `forward`, `callExport`, the
  `direct` list and the call-item branch of `deliver`.
- `web.rs`'s inline worker loses the same, and its `imports` resolver answers only
  `be3_host`.

The two workers then differ only in what the plugin worker adds (the GPU shim, WASI,
threads, dynamic linking), which is what `wasm-runner` is merging; whichever lands
second has less to merge. Threaded plugins keep their shared memory and wasi-threads.

## Performance

The GPU hot path does not change: local calls keep their flattened imports, and
nothing on that path reads the transport. `//crates/block-wasm-host:example-gpu-calls`
is the guard. There is no `be3.release` flag: a release build is the `-m release`
modifier (buck/constraints/BUCK, buck/platforms/modifiers.bzl), as
guides/performance.md already says:

    ./scripts/buck run -m release //crates/block-wasm-host:example-gpu-calls

Built that way on the build server in November 2026 (about two minutes, 98% cached),
it printed `100000 x (set pipeline, set viewport, set scissor, draw), 20 steps: fastest
129.8 ns per call, mean 325.1 ns per call`. The fastest matches the guide's 130 ns. The
mean is noisy on a shared VM, so compare fastest against fastest. No step below
touches the code it times.

**Former sync imports.** Today a sync import costs a flush (one `host_send` per queued
frame) plus one wasm import call that copies the args into an item with a ~40-byte
name. As an op it is a postcard `Start` with an 8-byte method id and an op id, sent
with the step's other frames by the same `host_send` per frame. It also costs an entry
in the guest's `calls`, and a `Complete` of about ten bytes coming back. There is one
`Complete` per notification, a few per frame (`frame-needed`, `layout`, input
answers), against hundreds of GPU calls.

**Former sync exports.** A sync export today costs `plugin_alloc`, a copy in, the call
and a copy out. As an op it is one copy in through `host_receive` and the `dispatch`
both paths already run; with the poll-once fast path, nothing is allocated for an
export that is ready at once.

None of these are measured as hot. If they turn out to be, the next saving is
`host_send` taking a batch of frames, for every op alike.

## Steps

Each lands alone, with `./scripts/verify` green, and leaves the guide describing what
is there.

1. **Kernel.** `Pending::detach`, and `dispatch` polling an async export's future once
   before boxing it. Tests: a detached call is not canceled and its `Complete` frees
   its entry; an async export that is ready at once answers in the same step without a
   task. Adds ~40 lines.
2. **block-type-api.** Mark host.wit's 8 imports and replicas.wit's 14 exports
   `async`. Update block-type-guest (its host calls `.detach()`, its `replicas` impl
   returns `ready(..)`), be-types-host, host-types and the `Sim`'s host, which now
   replies. That world then has no sync funcs.
3. **block-plugin-api imports.** Mark the 50 imports `async`; update the guest call
   sites and the host arms listed above.
4. **block-plugin-api exports.** Mark plugin.wit's 19 exports `async`; update the
   plugin SDK's impls and the host callers. Steps 3 and 4 can land in either order.
5. **Delete the direct path.** No WIT has a non-local sync func any more, so:
   - the generator refuses one everywhere (the network world's refusal, generalized,
     with its test renamed);
   - delete `Shape::Direct` and its arms, the import shims and registrar, the export
     shims, `export_<world>!` and its two call sites, `plugin_alloc`, `embed::IMPORTS`
     and `accept_direct`;
   - in be-ops delete `guest::direct`, `flush`, `Imports`, `register_imports`,
     `export`, `alloc`, `RESULT`, `Inner::call_direct` and `returning`, `Called`,
     `Incoming::Called`, `Connection::call_direct`/`notify_direct`/`return_direct`
     and the `direct` queue;
   - in block-wasm-host delete `direct.rs`, `Host::with_direct` and the call-item
     branch of `Plugin::deliver`;
   - in the web workers delete the call-item code in `plugin.js` and `web.rs`, and the
     `imports` message;
   - delete `block_guest_abi::direct`.
6. **Guide.** Rewrite guides/type_modules.md's "Operations and WIT":
   - the direct-item paragraph, "The rule" and the closing three-ways summary become
     the rule above;
   - add `detach` beside `Pending`.

   Drop `export_<world>!` from guides/adding_a_plugin_editor.md and
   guides/plugin_libraries.md where they name it.

## Risks and open questions

- **An export can now wait.** A former sync export could only run to completion, and
  the host relied on that for order: `screens` is applied before the frames after it.
  As an async func it may await, and frames read after it then run before it finishes.
  The implementations that land in steps 2 and 4 return `ready(..)`, which keeps the
  order exactly. Awaiting inside one later is a choice the WIT now shows.
- **Unanswered replies.** A host arm that forgets `reply.send` leaves a guest entry
  until the connection closes. A former notification's host arm answers first thing,
  so this is easy to review. A generated `Reply` that answers `()` when dropped would
  hide it, which is the hidden behaviour the rule is against, so it is not proposed.
- **Order of a former sync export against frames.** Today the runtime steps through
  earlier frames, then runs the export beside the step. As a frame it is read in order
  inside the step, which handles one item completely before the next, so the order is
  the same.
- **A plugin built before the change** stops loading against a host after it, and the
  other way round. Plugins are built from the same tree as their host, and the plugin
  protocol is not a frozen format (guides/frozen_formats.md).
- **Open: a GPU over frames.** wgpu's own web backend allocates ids on the client and
  never waits for a create. The GPU world could do the same: ops for its creates and
  draws, and await only for reads (`map_async`, `take-error`). It would leave no sync
  funcs anywhere, and batched it might beat 130 ns a call. But it moves validation
  errors away from the call that caused them and is a large change to
  block-gpu-guest. Not part of this plan; measure before considering it.

## Size

Removed, about 850 lines:

| Code | Lines |
|---|---|
| `block_guest_abi::direct` | 81 |
| block-wasm-host's `direct.rs` and its uses | ~100 |
| the call-item code in `plugin.js` | ~75 |
| the call-item code in `web.rs`'s worker, with its `imports` plumbing | ~85 |
| be-ops' direct paths in `lib.rs` and `guest.rs` | ~140 |
| the generator's `Shape::Direct` arms, import and export shims, `IMPORTS`, `export_<world>!`, `accept_direct` | ~250 |
| the hosts' `Called` branches | ~120 |

Added, about 250 lines:

| Code | Lines |
|---|---|
| `detach` and the poll-once fast path, with their tests | ~60 |
| a `reply.send(..)` in each of 58 host arms | ~60 |
| `ready(..)` and signature lines in 33 export impls | ~70 |
| `requested.rs` returning the reply | ~20 |
| test hosts answering | ~40 |

About 70 guest call sites also change in place to add `.detach()`. Net about -600.

Worth doing: it removes a whole wire format and the only JS that had to track a Rust
byte layout, and makes in-process and wasm guests take one path. It also makes every
WIT func say truthfully how it is answered, and guests can await a host's
acknowledgement where they could not before. It costs nothing on the GPU path, and
nothing is broken today, so it is not urgent.

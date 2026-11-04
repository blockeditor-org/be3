# Reactive core

`crates/beui-reactive` is a single-threaded reactive graph with no dependencies:
signals, memos, effects, and the scopes that own them. beui is built on it: the
`view!`/`#[component]` runtime in `crates/beui-tree` and the bindings to
`Document` nodes in `crates/beui-view/src/reactive.rs` are re-exported as
`beui::reactive`. The items carry doc comments with their invariants; this
guide is how they fit together. `./scripts/buck run
//crates/beui-reactive:example-retained` is a small complete program, and the
crate's tests are in `crates/beui-reactive/src/tests/`.

## Handles and ownership

Every handle - `Signal`, `ReadSignal`, `Memo`, `Selector`, `Stored`,
`StoredCell`, `KeyedStore`, a `#[derive(Store)]` store, and beui's `Callback`,
`Binding` and `NodeRef` - is a `Copy` key into a thread-local arena, not a
reference count (decisions/reactive_handles_are_copy_arena_keys.md). So a `move`
closure takes its own copy for free, and a struct made only of handles derives
`Clone, Copy` and is passed by value. A closure that needs an owned value (a
`String`, a `Prop<String>`) clones it in a block around the closure:
`create_memo({ let label = label.clone(); move || named(label.get()) })`.
State several closures share that is not reactive goes in one `store(value)`
rather than a clone per closure.

A handle belongs to the scope that was current when it was made and is freed
when that scope is disposed. Creating one with no current scope panics. Outside
a scope, state is owned one of three ways: `OwnedSignal`, `OwnedStored` and
`OwnedKeyedStore` free their value when they drop; `RootScope::new()` makes a
detached scope current for the rest of the block (tests start with
`let _root = RootScope::new();`); or `Scope::run` enters an existing one.

A long-lived scope keeps everything created in it until disposed, so state
that adds and removes items gives each item an `OwnedSignal` or a scope of its
own, or it grows without bound.

Scopes form a tree, and each has exactly one owner:

- `child_scope(f)` lives exactly as long as the current scope. Every
  `#[component]` runs in one, so a component lives as long as its caller.
- A `Scope` value disposes its scope when dropped. Control flow keeps each
  branch's scope in a `store` beside the child it built; `KeyedItems` and the
  dock keep one per entry in a map.
- A `ScopeContext` is a weak handle that owns nothing. Inside a computation the
  current scope is its execution scope, thrown away on every rerun;
  `owner_scope()` is the scope that made the computation, which is how a list
  keeps per-item scopes across its own reruns.

A computation may read a value another scope owns. If that scope is disposed
mid-flush, its readers are not woken by the freed value and are normally
disposed later in the same flush; one still alive that reads it panics.

## Choosing a primitive

| Need | Use |
| --- | --- |
| State that changes | `create_signal`; `.read_only()` to hand out a `ReadSignal` |
| A value derived from others | `create_memo`: lazy, cached, and an equal output wakes nothing |
| Keep an existing node's property in step | `create_binding`: runs now, costs nothing if it reads nothing |
| A side effect, or work that needs the tree built around it (moving focus) | `create_effect`: queued, runs at the end of the entry |
| "Is this row the selected one?" across N rows | `create_selector`: wakes only the two rows whose answer flips |
| A struct whose fields change separately | `#[derive(Store)]`: a signal per field |
| A list whose rows bind to their own item | `KeyedStore`: a signal per item, one for the order, `reconcile` writes what differs |
| Shared non-reactive state | `store`, or `StoredCell` for a `Copy` value |

Reading and writing are the `Get` and `Set` traits, implemented by signals,
memos, stores, `Prop` and `Binding`; import them with the types, and take
`impl Get<T>` in a helper that only reads. `get`/`with` subscribe the running
computation; the `_untracked` forms and `untrack(|| ...)` do not. Dependencies
are rediscovered on every run, so a branch not taken unsubscribes.

## When things run

A write changes the value at once, so a read right after it sees the new value,
and the memos over it are recomputed lazily when read. A write never runs an
effect. The effects it wakes are queued and run in exactly these places:

- when the outermost `Scope::run` or `ScopeContext::run` (an *entry*) ends;
  beui runs every event, timer, accessibility action and frame step in one,
  so a write in a click handler is visible in the same frame and to the next
  event;
- at `flush()`, which tests call after a write to check what an effect did;
- `ScopeContext::flush_owned` runs only one scope's subtree, which is how a
  node builds its own subtree during layout;
- when a zone with parked effects is entered (below).

Creating an effect, reading a memo, disposing a scope and resuming one all
leave effects queued. Effects queued outside any entry (a
write from a future or a host callback) call the waker set with `wake_with`
once, so a burst asks for one frame; beui's `Context::set_waker` sets it.

So code outside a beui event that writes and then expects an effect's result
without a `flush()` sees the old state.

## Zones

`enter_zone(zone)` enters a zone until its guard drops (zone 0 when none is).
An effect remembers the zone it was created in; woken outside it, it is parked
until that zone is entered again. beui gives each `Document` a zone and enters
it while the document is installed. That is what makes a signal safe to write
from anywhere on the UI thread: a write made while another document, or none,
is installed does not run this document's effects outside its frame. Each
document registers `wake_zone_with` with the `Context` showing it, which passes
it to the platform's `Waker`, so the window draws a frame without the writer
asking. A wake raised during a frame asks for another only if the zone still
has effects parked at its end.

## Pausing

A pausable scope (`Scope::detached().pausable()`) holds back the effects under
it while paused and runs each woken one once on `resume()`. An effect already
woken when it pauses still runs, so the write that hides a subtree reaches it
(an overlay inside closes). `ShowKeepAlive` pauses its hidden child this way;
`Scope::pausable_effects()` pauses effects but not bindings, for `Loading`.

## Loading

Data that may not have arrived is a `Ready<T>` (`Result<T, Pending>`, with
`Pending::Loading` or `Pending::Failed(message)`), and a memo derives from it
with `?`: `create_memo(move || Ok(rows.get()?.len()))`. No `Option` or
`loaded: bool` sits beside the value
(decisions/loading_state_is_ready_and_waits_at_a_boundary.md).

`<Loading fallback={...}>` builds its content at once but hidden, shows the
fallback while anything that content made is pending, then reveals it and
never shows the fallback again, so a later reload updates in place. A source
registers with the nearest boundary around the scope that makes it, through
`wait_for(probe)` or `hold(source)`; a `Memo<Ready<T>>` handed to a `Prop<T>`
is held that way, so `<Text>{name}</Text>` takes one directly. Traps:

- Nothing registers inside a memo, which reruns on every read. Make the source
  in a body or a binding.
- A wait lasts as long as the scope that registered it. A source a binding
  caches across its runs is made in a scope kept beside it, not the binding's
  own, which the next run drops.
- While hidden, the content's bindings, memos and control flow run, so the
  boundary finds every source; its effects wait until the reveal. An effect
  that makes a source the view waits for (one that watches a new query when
  an id changes) has to be a `create_binding`, or the boundary never sees it
  and never reveals.
- A source made outside the boundary (in `view` around `<Loaded>` instead of
  inside its content) does not hold it back.

## Blocks

An editor reads block content through a `ContentProjection` (`be_sdk_beui`):
`editor.block_content::<C>()` for its own block, `editor.content_of::<C>(block)`
for another. These are be-sdk-beui's signals over be-sdk's `LiveContent`, which
has none of its own (guides/the_new_block_stack.md, "State and change
callbacks"); `Blocks` and `BlockList` do the same for the block graph. The app
framework pumps them at the top of every frame. A projection is a `Copy` handle
the `Editor` owns; one made outside an editor is an `OwnedContentProjection`.

A projection is a pure function of the content as the view sees it now, not of
the operations applied to it: with an edit in flight, the visible content is
rebuilt from the confirmed content plus pending edits and can move in ways no
single operation describes. It runs only when an operation touches what it
watches. On a `be-block-model` document, narrow it:
`field(object, FIELD)` watches one field, `ids(owner, LIST)` one list's order,
`object::<T>(id)` one object, `objects(owner, LIST)` a list's objects behind
`Rc`s replaced only for the objects an edit touched. Reading one field through
`root()` decodes the whole document. The checklist's rows each watch their own
item, so ticking one runs one row.

Writes go through `ContentProjection::operate`, which applies to the visible
content at once and queues the operation for the replica. It is dropped while
the block may not be edited and until the content has arrived, since an edit
against missing content would land in the wrong place. Graph queries
(`editor.watch_blocks(query)`) are `Ready` memos too.

An editor shows nothing it would correct a moment later: no empty or editable
view before the content arrives, no placeholder the next answer replaces. Each
`BeuiRoot::View` and `Preview` wraps its component in `<Loaded label="...">`,
the editor's `Loading` boundary, which waits on every projection, graph query
and `version_status()` made inside it. Something that loads later inside a
shown view (a folder being expanded) says so in its own place.

## beui integration

Nothing in a view takes `&mut Document`: `build` and every event install the
document in a thread-local that `with_document` reads, so ambient calls nest. Only the base elements and the structural
primitives (`show`, `dynamic`, `keyed`, `for_each`, `VirtualList`) touch
`Document`; everything above says what it wants through props.
[beui/core.md](beui/core.md) ("Writing views") covers slots, child types,
modifiers, value components and `ComponentRef` handles; the `view!` grammar is
`crates/beui-macros`, with each compile error the types enforce in a case
under `compile_fail/beui/`.

### No tracked reads in a component body

A component body runs once with nothing observing it, so a `get` or `with`
there would never update. In debug builds it panics, naming the type, even on a
`Prop` holding a plain value (the same component may later get a signal). Move
the read into a memo, binding or effect; pass the signal or memo on as the prop
itself; or use `get_untracked`/`untrack` for a deliberate snapshot. A helper
called from a body trips the same way. `Prop::map` runs at once on a plain
value, so a mapping that reads other signals is
`Prop::dynamic(move || tab_stop.get() && !disabled.get())`.

### Props, bindings and controlled state

A `Prop<T>` takes a plain `T` or a signal or memo of one; the base components
bind it with `create_binding`, so a plain value costs nothing after it is set.
Pass a memo to a prop as it is. Every prop is required unless it has
`#[prop(default = ...)]`, or is `Option<Prop<T>>` (absence fixed at build) or
`Prop<Option<T>>` (can return to "unset" while alive, such as a `Frame` width).
`Callback`, `Children` and `Binding` props default to empty.

State a control both shows and changes is one `Binding<T>`, not a prop plus an
`on_change`. Left out, the component owns a signal seeded from its default;
given a `Signal<T>`, it reads and writes that; given `Binding::new(read, write)`,
it shows `read` and calls `write`, so a write the callback ignores changes
nothing on screen. React to a change by deriving from the signal, not by asking
for a callback. Reporting callbacks (`on_hover_change`, `on_focus_change`) take
a signal too.

Base-element state that the document can also change on its own - `focused`
on an `Interactive`, `open` on an overlay, `offset` on a scroll - is
edge-triggered: the node acts when the signal changes. Pair it with the
matching callback (`on_focus_change`, `on_dismiss`) and write the signal back,
or it goes stale and the next write of the value it already holds does
nothing. A `Selector` is the natural source of `focused` in a list.

### Children, render props and control flow

`view!` builds a tag's children *before* the tag's own body runs, so a subtree
passed as `Children` is already built when the parent starts. Anything that
must happen first - providing context, measuring a container - takes the
subtree as a render prop (`Render`/`RenderFn`, marked `#[prop(children)]` so
callers still write it between the tags) and calls it afterwards. A render
prop runs with the component that *wrote* it installed, so its state and
accessibility belong to the outer component. A slot that hands a value names
it like a closure argument, `<Container>|size| <Card size /></Container>`; a
`RenderFn` slot may run more than once, so its body clones what it moves.

`Show`, `Dynamic` (rebuild on every value change), `Keyed` (rebuild when its
`key` changes, update the rest through a `ReadSignal`) and `ForEach` build no
node: each fills a run in its parent's slot and builds each child in a scope of
its own, disposed with the child. Key a `ForEach` by identity, never by
content: a key that changes with an edit destroys the row's nodes and scope and
loses its focus, caret and measured text. Give the row the key and let it read
its own item from a `KeyedStore`.

### Context

`provide_context(value)` stores a value on the current scope by its type, and
`use_context::<T>()` finds the nearest. Component scopes follow the node tree
and a computation's execution scope is parented to its creator, so an effect
that builds a `Show` branch long after the first frame still sees what was
provided above it. Use a wrapper type per concern; two providers of a bare
`f32` collide. `narrower_than(width)` answers for the nearest `unstyled::container`; in
debug builds it panics with none, which usually means the children were built
in the caller's scope rather than inside the container's render prop.

### Nodes and scopes

A node belongs to the scope current when it was made, like an effect, and
disposing that scope removes it; a scope's nodes, effects and cleanups go
together. Code in `build`'s closure belongs to the document's root scope
(`Document::reactive_scope`). Anything that drops part of a view on its own
keeps a `Scope` for it and drops that.

The owning scope and the holding parent can differ: removing a node detaches
children owned by another live scope (what lets a `Portal` show a subtree built
elsewhere). Removal is
deferred: each node's cleanup queues its id with its document, which removes
them after each event, after layout and when `with_reactive_scope` returns, so
nothing leaves the arena mid-walk.

An effect that writes one node belongs to that node: `bind(node, || ...)`
creates it in the node's scope, so it stops when the node goes. A `NodeRef`
lets go of a removed node, so `try_get` finds nothing rather than a stale id.

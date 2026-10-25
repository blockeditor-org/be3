# Reactive core

`crates/reactive` provides a dependency-free, single-threaded reactive graph. It
is intended for retained UI bindings: create a node once, then use an effect to
update its properties when the values it reads change. `beui::reactive` (in
`crates/beui-tree` and `crates/beui-view/src/reactive.rs`) is the adapter that
connects it to beui's components and `Document`; see "beui integration" below.

## Example

```rust
use reactive::{create_effect, create_memo, create_signal, Get, Scope, Set};

let scope = Scope::new();
let count = scope.run(|| create_signal(0));

scope.run(|| {
    let text = create_memo(move || format!("Count: {}", count.get()));
    create_effect(move || println!("{}", text.get()));
});

count.set(1);
count.update(|count| *count += 1);
```

Keep the scope alive for as long as the view exists. Dropping it or calling
`scope.dispose()` stops its computations, runs their cleanup callbacks, and
frees the signals and memos created in it.
Run `./scripts/buck run //crates/reactive:example-retained` for a complete example that
updates retained state while it holds a mutable borrow, then flushes.

## Signals and memos

`create_signal(value)` returns a `Signal<T>` that both reads and writes. It is
`Copy`, including for values that do not implement `Clone`, so a `move`
closure takes its own copy and the original stays usable. A handle is a key into
a thread-local arena, not a reference count: the signal belongs to the scope
that was current when it was created and is freed when that scope is disposed.
Creating one with no current scope panics. Reading a freed signal panics;
`set` and `set_unconditionally` on one do nothing, since nothing can read the
value they would write, and `is_alive()` asks whether it is still there.
Handles and scopes cannot cross threads.

Reading and writing are two traits, which `reactive` and `beui::reactive`
export: `Get<T>` (`get`, `with`, `get_untracked`, `with_untracked`) is
implemented by `Signal`, `ReadSignal`, `Memo`, `Prop` and stores, and `Set<T>`
(`set`, `update`) by `Signal` and stores. A file that calls them imports the
traits along with the types, and a helper that only needs to read or write any
of them can take `impl Get<T>` or `impl Set<T>`.

- `get()` clones a value and subscribes the current computation.
- `with(|value| ...)` borrows it and subscribes without requiring `Clone`.
- `get_untracked()` and `with_untracked(...)` read without subscribing.
- `set(value)` requires `PartialEq` and notifies only when the value changes.
- `update(|value| ...)` mutates in place, returns the closure's result, and always
  notifies. It works without `PartialEq` or `Clone`.
- `set_unconditionally(value)` replaces and always notifies.

`signal.read_only()` returns a `ReadSignal<T>`: the same signal, without the
writes. Hand one out where the receiver must not write, such as a struct's
public field whose writes go through the struct's methods, or a state a
component publishes for tests.

`create_memo(|| ...)` creates a read-only cached computation with `PartialEq`
output. It computes once immediately, then refreshes on demand. A memo whose
first computation reads no signal and leaves nothing to clean up is a constant:
it keeps its value and no computation at all. Memos expose the
same reading methods as signals, are `Copy` the same way, and are freed with
their scope. A changed dependency invalidates downstream
computations, but equal memo outputs suppress downstream execution. Reading a
memo whose effects have not run yet returns its current value. Dependency chains and diamonds
refresh before effects observe them, preventing intermediate derived values.

Dependencies are discovered on each execution. Conditional branches unsubscribe
from inputs they no longer read. `untrack(|| ...)` disables subscription for its
closure while preserving the current cleanup scope. Memo computations must be
pure: writing a signal inside a memo panics, including inside `untrack`, and
`./scripts/verify` reports a `set`, `update` or `set_unconditionally`
written inside a `create_memo` closure outside tests.

`with` holds a shared borrow for the closure; `update` holds a mutable borrow.
Do not access the same signal incompatibly from those closures.

## Owning signals outside a scope

State that outlives any one scope, or is made where no scope is current, owns
its signals one of three ways:

- `OwnedSignal::new(value)` is a signal that belongs to the value holding it and
  is freed when that value drops. `handle()` hands out its `Signal<T>`, and it
  derefs to one, so `owned.get()`, `owned.set(value)` and `owned.read_only()`
  work on it directly. Use it for a signal per entry in a map that outlives its readers
  (`KeyedStore` keeps one per key, and `Document` one per watched node), or for
  a struct field when the struct is made outside a scope.
- `RootScope::new()` makes a detached scope current until it is dropped, so
  everything created after it in the same block belongs to it. A test that
  creates signals before building a document starts with
  `let _root = RootScope::new();`. A constructor that creates many signals
  starts the same way and ends with `owner.into_scope()`, which stops it being
  current and returns the `Scope` for the struct to keep. Unlike `Scope::run`,
  it is not an entry: it flushes nothing when dropped. Drop or hand back a `RootScope` in the block
  that made it.
- `Scope::run` creates them in an existing scope.

A long-lived scope that keeps creating signals keeps all of them until it is
disposed, so state that adds and removes items over its life gives each item an
`OwnedSignal` or a scope of its own rather than creating them in the long-lived
scope.

## Stored values

`store(value)` puts any value in the same arena as signals and returns a
`Stored<T>`: a `Copy` handle owned by the current scope and freed with it. Use
it for state a component's closures share that is not itself reactive: an
editor's state struct, a `Cell` a gesture writes, a map of children. A struct
whose fields are signals, memos, selectors, `Stored` values and other `Copy`
handles derives `Clone, Copy` and is passed around by value, so closures take
it with `move` and nothing clones it.

- `with(|value| ...)` borrows the value; `value()` returns its `Rc<T>`. Both
  panic once the scope is disposed. `try_with`, `try_value` and `is_alive`
  don't. Neither tracks: a stored value is not reactive.
- It holds the value immutably; put a `Cell` or `RefCell` inside for state that
  changes, or use a signal when readers must hear about the change.
- `OwnedStored::new(value)` is the scope-free counterpart of `OwnedSignal`: it
  frees the value when it drops and hands out `Copy` handles with `handle()`
  (it also derefs to one). Use it for state made outside a scope, such as an
  editor the host keeps.

`Callback`, `ClickCallback`, `Binding` and `NodeRef` are `Copy` the same way. An empty
callback needs no scope; `Callback::new`, `ClickCallback::new` and
`NodeRef::new` create stored values, so they need one. A `Callback<T>` made from
a `Signal<T>` (`Callback::from(signal)`, or the signal passed where a callback
is expected) holds just the signal's handle and sets it when called. Calling a callback
whose scope is gone does nothing, as `set` on a freed signal does. A callback
runs its handler untracked, so an effect that calls one subscribes to nothing
the handler reads.

## Cloning handles into closures

Signals, memos, selectors, stored values, callbacks, node refs, and structs
made of them are `Copy`, so a `move` closure takes them without ceremony, and a
closure that captures only `Copy` values is itself `Copy`. A closure that needs
its own copy of an owned value (a `String`, a `Vec`, a `Prop<String>`) clones
it in a block around the closure:

```rust
let accessibility = create_memo({
    let label = label.clone();
    move || named(label.get())
});
```

A value several closures share is better stored once with `store` than cloned
into each. For a `Copy` value they read and write, such as the point a gesture
started at, `StoredCell::new(value)` is a `Copy` cell with `get`, `set`,
`replace`, `take` and `update`, owned by the scope like a `Stored`.

## Selectors

A list of N rows that each ask "am I the selected one?" through a memo wakes all
N of them every time the selection moves. `create_selector(|| ...)` turns that
into two: it keeps one subscriber list per key that has been asked about, and
when its source changes it notifies only the key that lost selection and the key
that gained it.

```rust
let selected = create_signal(Some(0usize));
let selection = create_selector(move || selected.get());

for index in 0..rows {
    create_effect(move || highlight(index, selection.is_selected(&Some(index))));
}
```

`is_selected(&key)` subscribes the current computation to that key alone, never
to the source, so a computation reading it reruns only when its own answer
flips. `memo(key)` wraps one key in a `Memo<bool>` for props and handles that
want a value rather than a call. Keys need `Clone + Eq + Hash`, and the source's
own value is its key type, so an optional selection is a `Selector<Option<K>>`
queried with `is_selected(&Some(key))`.

Reads are current the moment they happen, including before the effects of a
write have run, and a key is forgotten once the last computation watching it is
disposed or stops reading it. Like memos, a selector must be created inside a
scope, and its per-key notifications stop when that scope is disposed.

## Stores

A signal holding a struct is one source for every field in it, so writing any
field wakes every computation that reads any other. `#[derive(Store)]` splits it:
for `Foo` it writes a `FooStore` whose fields are a `Signal` per field of
`Foo`, with the field's visibility, and a `new(value)` that seeds them. The
store implements `Get<Foo>` and `Set<Foo>`: `get()` reads the whole struct
back, and `set(value)` writes each field (so a field whose value did not change
notifies nobody). The store is `Copy`, and its signals belong to the scope it
was created in.

```rust
#[derive(Clone, PartialEq, Store)]
pub struct Theme {
    pub background: Color32,
    pub accent: Color32,
}

let theme = ThemeStore::new(Theme::DARK);
let accent: Prop<Color32> = theme.accent.into_prop();
theme.accent.set(Color32::WHITE);
```

Each field is a `Signal<T>`, so it goes straight into a `Prop<T>`. A helper
that picks between fields takes the store and reads the one the branch it takes
needs, which is narrower still: tracking is per execution, so a memo around it
subscribes to that field alone and re-subscribes when the branch changes.

```rust
fn fill(theme: &ThemeStore, hovered: bool) -> Color32 {
    match hovered {
        true => theme.accent.get(),
        false => theme.background.get(),
    }
}
```

`beui::styled::use_theme` returns the document's `ThemeStore`, which is why a
component binds a colour rather than the theme.

## Keyed stores

`KeyedStore<K, V>` is the same idea for a list: one signal per item, plus a
signal holding the order. `reconcile` takes the items as they are now, writes
only the ones whose value differs, writes the order only when it differs, and
forgets the items whose keys are gone.

```rust
let items: KeyedStore<Uuid, Item> = KeyedStore::new();
items.reconcile(list.iter().map(|item| (item.id, item)));
```

A `KeyedStore` is a `Copy` handle owned by the scope that made it, like a
signal; `OwnedKeyedStore::new()` is the one for a store made outside a scope,
which frees it when it drops and derefs to the handle.

Values that are already owned go through `reconcile_owned`; `reconcile` takes
them by reference and clones only the ones it is about to write. `keys()` is the
order signal — read it with `with` rather than `get` so the list is not cloned —
and `get(&key)` is that item's signal, which stays the same handle across
reconciles.

That handle is what a row binds to, which is why `for_each` takes keys rather
than values:

```rust
<List spacing=8.0>
    <ForEach keys={visible}>|id: Uuid| {
        let item = items.get(&id);
        view! { <ItemRow item /> }
    }</ForEach>
</List>
```

Give the row the key and let it read its own item. A key that contains the row's
content — the whole item, or its text — changes whenever the content changes,
which destroys the row's nodes and its scope and builds a replacement, losing
focus, caret and measured text along the way. A key that is the item's identity
survives the edit, and only the bindings that read what changed run.
`for_each` reconciles its children in place, so a list whose order did not change
leaves the document untouched no matter how much of its content did. A key
that appears more than once gets a row for each appearance, matched to the
appearances in order, so a list of keys is never wrong, only slower to edit
when it repeats one.

## Blocks

A plugin editor reads its block's content through a `ContentProjection`
(`block_editor_beui`). `editor.block_content::<C>()` is the one for the
editor's own block, `editor.content_of::<C>(block)` the one for another block it
follows; both are pumped by the plugin framework once at the top of every frame,
so an editor never pumps by hand. A
projection is a `Copy` handle that the `Editor` owns and frees with itself, so
closures take it with `move`. One made outside an editor, as
`EditorHost::content_of` makes one for an artifact's regeneration, is an
`OwnedContentProjection`, which frees it when it drops and derefs to the handle.

```rust
let checklist = editor.block_content::<ChecklistContent>();
let done = checklist.project(|checklist| checklist.root().done_count());
let ids = checklist.ids(ObjectId::ROOT, Checklist::ITEMS);
let item = checklist.object::<ChecklistItem>(id);
let count = counter.field(ObjectId::ROOT, Counter::COUNT);
```

A projection is a plain function of the content as the view sees it now. It
writes its signal, which notifies only when the value it computed actually
changed. Deriving from the current value rather than from the operations applied
to it is what makes this correct: when an edit from elsewhere arrives while one
of this editor's own is still in flight, the visible content is rebuilt from the
confirmed content and the pending edits, so it can move in ways no single
operation describes.

A projection also only runs when something it watches was touched. Applying an
operation reports what it touched, and `project` watches everything,
`project_on(key, ...)` one key, and `project_keyed` fills a `KeyedStore`. On a
`be-model` document the projection can be narrower still: `field(object, FIELD)`
runs for one field of one object, `ids(owner, LIST)` for the order of one list,
`object::<T>(id)` for one object and everything inside it, and
`objects(owner, LIST)` for every object of one list, each behind an `Rc` that an
edit replaces only for the objects it touched, so a list of hundreds re-reads one
when one changes. The checklist's rows each watch their own item and its list
watches only its order, so ticking one item runs one row. Read a single field
with `field` rather than through `root()`, which decodes the whole document. A watcher is dropped with the reactive scope that made it.

Keep a projection cheap and pure; it must not operate. Write through
`ContentProjection::operate`, which applies the operation to what the view
sees straight away, so a local edit is visible in the frame that made it, and
queues it for the block's replica (a `FromEditor::Operate` frame; the projection is
`block_type_guest::editor::EditorMirror` plus the watchers). It is already held behind whether the host says the
block may be edited, and dropped until the host has sent the content once, since
an edit made against content that has not arrived would land in the wrong place.
`read` answers the content outside a projection, `Err(Pending::Loading)` until
the host has sent it once, and `ready()` is that state on its own.

An editor shows nothing it would have to correct a moment later: not an empty
or editable view before its content arrives, and not a placeholder ("no rows")
that the next answer replaces. Every `BeuiApp::View` and `Preview` wraps
its component in `<Loaded label="...">`, the editor's `Loading` boundary (see
Loading, below): it waits for whatever its content reads. The sources that
wait are the editor's own: a projection of a block's content
(`block_content`, `content_of` and everything built on them, for another
block until its content arrives or the account turns out not to be able to
read it), a graph query (`editor.blocks().watch(query)`, `watch_blocks`),
`version_status()`, and the database helpers `watch_inverses` and
`watch_row_labels`. So the sources must be made inside the boundary's
content, not in `view` around it. One with a placeholder that is already
right while loading shows that instead (an image shows its thumbhash).
Something that loads later, inside a view that is already shown (a folder in
the file tree being expanded), says so in its own place, from its `Ready`.

The graph around a block - its children, what it references, what references
it - is a query rather than content. `editor.watch_blocks(BlockQuery::Children(id))`
is a memo of the host's answer to that query: `Err(Pending::Loading)` until the
first answer arrives, `Err(Pending::Failed(..))` when the host could not answer
it, and it changes whenever the host's answer does. A query nobody watches
keeps its last answer for a while (the plugin keeps the last 64), so watching
it again shows that at once while the host answers again.

## Loading

Data that may not have arrived is a `Ready<T>`, `Result<T, Pending>`, where
`Pending` is `Loading` or `Failed(message)`. A memo derives from one with `?`:
`create_memo(move || Ok(rows.get()?.len()))`. Nothing in between keeps an
`Option` or a `loaded: bool` beside the value.

`<Loading fallback={|pending: Pending| ...}>{content}</Loading>` turns that
state into UI. It builds its content at once but keeps it hidden, and shows
the fallback while anything the content made waits; once nothing does it
reveals the content, and never shows the fallback again, so a later reload
updates the view in place. A source waits on the nearest boundary around the
scope that makes it: `wait_for(probe)` registers a `Fn() -> Ready<()>` and
`hold(source)` turns a `Ready<T>` into a memo of the last `T` it was (the
default before the first) that waits for it. A `Memo<Ready<T>>` handed to a
`Prop<T>` is held that way, so `<Text>{name}</Text>` takes it directly.
Inside a memo nothing registers: a memo runs whenever it is read, so it would
register again each time; make the source in a body or a binding instead. A wait
lasts as long as the scope that registered it, so a source a binding caches
across its runs is made in a scope of its own, kept with the cached source,
rather than in the binding's, which the next run drops. Once a boundary has
revealed nothing registers with it any more. An
inner `Loading` keeps what its own content waits for from the one around it.

While the content is hidden its bindings, memos and the controls that build
the tree (`Show`, `Dynamic`, `ForEach`, `Keyed`) run, so the boundary finds
everything the content will show. Its effects wait (a scope made with
`Scope::pausable_effects()`) and run once at reveal, with the data that
arrived, so an effect that writes outside the graph - an intrinsic size,
seeding a child block - never acts on data that has not arrived. That makes
the split matter: an effect that makes a source the view waits for, such as
one that watches another query when an id changes, is a `create_binding`.

## Effects, entries, and ownership

Create signals, memos, effects, and cleanup callbacks inside `Scope::run` or
another computation. `create_effect(|| ...)` queues an initial execution, then runs
again when its tracked inputs change.

A write never runs effects itself. The value changes at once, so a read right
after it, of the signal or of a memo over it, sees the new value; the effects
it wakes are queued. Queued effects run in exactly three places: when the
outermost `Scope::run` or `ScopeContext::run` ends, when a frame begins or
ends, and at `flush()`. Every event, timer, accessibility action and frame
step beui handles runs inside a `ScopeContext::run`, so its effects run once
at its end. Nothing else runs them: creating an effect or a binding, reading
a memo, disposing a scope and resuming a paused one all leave them queued.
(Effects parked for a zone are the one addition: they run when that zone is
entered, see "Zones".) Whenever effects are left queued outside any entry, from
a write in a future or a callback the host runs, or from creating an effect,
the runtime calls the waker registered with `wake_with(wake)`, once until the
next flush, so a burst of them asks for one frame; beui's `Context::set_waker`
registers it. `flush()` runs everything queued there and then, for the rare
caller that needs effects to have run, such as a test that writes and then
checks what an effect did; it does nothing inside an effect, whose flush is
already running. Multiple writes coalesce into one pending
execution. Effects created inside effects run after their parent finishes.

An effect that finishes its first execution without reading a signal, opening a
scope, or registering a cleanup can never run again, so it is disposed right
there and releases whatever its closure captured.

`create_binding(|| ...)` is an effect whose first execution happens at once,
before the entry around it ends, the way a memo's does. If that execution read
nothing and left nothing to clean up, no computation is made at all; otherwise
it reruns like an effect. Bind a property of a node that already exists with
it, as the base components do with their props: most props are plain values,
and a binding of one costs only its closure. Each binding still costs a probe,
so bind props that are applied together in one binding, as `Text` binds its
whole `TextStyle`; the setters skip a value that has not changed, so rerunning
the group for one prop touches nothing else. Anything that has to wait for the
tree it is in to be built, such as moving focus, stays an effect.

`create_effect` returns an `Effect` handle for explicit `dispose()` and
`is_disposed()` checks. Dropping this handle does not stop the effect; its scope
owns it. Effects may write signals, and resulting work is queued rather than
recursively executed. An effect exceeding 10,000 executions in one flush panics and is disposed
to stop runaway feedback loops.

A `Scope::new()` created inside an active scope becomes its child. Parent disposal
also disposes children, even when a child handle remains alive. A scope cannot
be entered after disposal, and disposing it frees the signals, memos and
selectors it owns. Disposal is idempotent.

A scope has exactly one owner, and there are two kinds:

- `child_scope(|| ...)` runs its closure in a new child of the current scope
  that lives exactly as long as that scope, and returns what it built with a
  `ScopeContext` to enter it again later. Every `#[component]` runs in one.
  It panics with no current scope, since nothing would ever dispose it.
- A `Scope` value owns its scope and disposes it when dropped.
  `Scope::detached()` opens one under the current scope, so it reads that
  scope's context and pauses with it, but only the value disposes it;
  `Scope::new()` is also disposed with the current scope. Keep the value where
  its lifetime belongs: in a `store` when it should go with the current scope
  (the control-flow components keep each branch's scope in one, beside the
  child it built), or in a map entry when it goes with the entry, as
  `KeyedItems` and the dock's panels do.

A `ScopeContext` is a weak handle that owns nothing. Inside a computation, the
current scope is that computation's execution scope, which is thrown away
every time it reruns; `owner_scope()` returns the `ScopeContext` of the scope
that created the running computation (or the current scope when no computation
is running). `ScopeContext::child()` opens a scope under it that its handle
owns, returning `None` if that scope is already gone, and `is_alive()` answers
the same question on its own. This is how a list keeps per-item scopes across
updates of the list itself while still reading what was provided above it.

`on_release(target, key)` registers `target.release(key)` instead, for a
`Release` target shared by many registrations, so it allocates nothing: beui
queues a node's removal this way for every node a scope owns.

`on_cleanup(|| ...)` registers a callback on the current scope. Every computation
has a fresh execution scope: before it reruns, its old nested computations and
cleanups are disposed. Cleanups run in reverse registration order without
tracking reads. All cleanup callbacks are attempted even if one panics; the first
panic is propagated unless disposal is already unwinding.

Tracking, ownership, and scheduler state are restored when user code panics.
A `Scope::run` that panics does not flush effects while unwinding; pending work
runs at the next flush. A panicking `update` invalidates its
value because it may already have mutated it. There is no transaction rollback.

## Zones

`enter_zone(zone)` enters a zone until the guard it returns is dropped. While
no zone is entered, zone 0 is. An effect remembers the zone that was entered
when it was created, and the effects it creates inherit it. An effect woken by
a write runs only while its own zone is entered; otherwise it is parked, and it
runs when its zone is next entered or returned to as an inner zone's guard
drops. So an effect made outside every zone runs at the next flush outside every
zone, but never in the middle of a document's frame: it waits until the
document's zone is left. `zone_pending(zone)`
says whether a zone has parked effects, and `forget_zone(zone)` drops them.
`wake_zone_with(zone, wake)` registers a callback that runs when a zone with
nothing parked parks its first effect, so a burst of writes wakes it once.

beui gives each `Document` a zone of its own and enters it whenever the
document is installed, which is what lets a signal be written from anywhere on
the thread: a write made while another document is installed, or none, does
not run this document's effects outside it, and the document catches up the
next time it is shown. Each document registers a wake for its zone with the
`Context` that shows it, and the context passes it to the adapter's `Waker`,
so the window draws a frame without the writer asking. A wake raised during a
frame is held until the frame ends, and asks for one more frame only if the
zone still has effects waiting, since a document shown later in the same frame
runs them itself.

## Pausing

`Scope::detached().pausable()` (or `Scope::new().pausable()`, before anything
is created in it) is a scope that can be paused. While `pause()` holds, an
effect created under it that is woken waits instead of running, and
`resume()` queues each one that waited to run once at the next flush, with the
values as they are by then. Memos are not paused, but they only compute when read, so a memo only
paused effects read waits with them. An effect already woken when the scope
pauses still runs, so the write that hides a subtree reaches it: an overlay
inside closes rather than staying open. A scope inside a paused scope waits
for it, whether or not it is paused itself. This is what keeps a hidden
`ShowKeepAlive` child from doing work nobody can see. `Scope::pausable_effects()` pauses only its effects,
leaving its bindings running; it is what a `Loading` boundary keeps its hidden
content in.

## beui integration

`beui::reactive` (re-exporting `create_signal`, `create_effect`, `create_binding`, `create_memo`,
`create_selector`, `store`, `Scope`, `flush`, `untrack`, `on_cleanup`,
`provide_context`, and `use_context` from this crate) binds signals and
memos directly to `Document` nodes. Nothing in a view takes a `&mut Document`
parameter, so tags nest the way JSX or solidjs would nest them: write the tree
with `view!` inside a `#[component]`, and build the document with `build`, which
supplies the document ambiently and returns the finished `Document`.

```rust
use beui::reactive::*;
use beui::styled::*;

#[component]
fn App() -> beui::NodeId {
    let count = create_signal(0i64);
    let count_text = create_memo(move || count.get().to_string());
    view! {
        <List spacing=0.0>
            <List direction=Direction::Horizontal spacing=8.0>
                <Button label="-" on_click={move || count.update(|count| *count -= 1)} />
                <Text>{count_text}</Text> // updates itself when `count` changes
                <Button label="+" on_click={move || count.update(|count| *count += 1)} />
            </List>
        </List>
    }
}

let document = build(|| view! { <Root><App /></Root> });
```

`#[component]` turns a function named `Name` into a `<Name>` usable from `view!`.
That exact function name is also the constructor that returns the builder `view!`
fills in. A prop typed `Prop<T>` accepts
either a plain `T` or a signal or memo of `T`; the base components bind each
prop with `create_binding`, which keeps a reactive form in sync and costs a
plain value nothing after it is set. Components return their base node
directly. Tags also accept `@` modifiers, ordinary functions applied to the
built value. The GUI prelude supplies `test_id`, `node_ref`, and `sizing`.

### No tracked reads in a component body

A component body runs once with no computation observing it, so `get` or
`with` there would subscribe nothing and the value it read would never update.
In debug builds such a read panics, naming the type read: on any signal, memo,
store, selector key or `Prop`, including a `Prop` that holds a plain value,
since the same component may later be handed a signal. `component()` marks the
body with `reactive::component_body`, and the mark is lifted inside `untrack`
and inside every effect, memo and binding, so reads there track as usual.
Event handlers, timers, host code and tests run outside any body. Fix a read by:

- moving it into a closure, memo or effect, so it updates;
- passing the signal or memo itself on as the prop;
- using `get_untracked()` or `with_untracked()` (or `untrack`) when a one-time
  snapshot is intended.

A helper that reads signals, called from a body, trips the same way; wrap the
call in `untrack` when its value is a snapshot. `Prop::map` runs its function
at once on a plain value, so a mapping that reads other signals is a
`Prop::dynamic` that reads the prop too:
`Prop::dynamic(move || tab_stop.get() && !disabled.get())`.

## Writing attributes

An attribute value in braces is any expression at all, and that is what closures,
blocks, method chains, and anything containing `<` or `>` need. Values simple
enough to read on their own may drop the braces: literals, paths, a call on a
path, a unary expression, and a reference to a path.

```rust
<Frame fill=ColorRole::Surface>
<Text style=TextStyle::CAPTION align=TextAlign::End>{count_text}</Text>
<List direction=Direction::Horizontal />
<Frame @node_ref=&panel width=TRIGGER_WIDTH height=HEIGHT>
<Frame fill=ColorRole::Accent radius=Radius::Sm>
```

An attribute written as a bare name takes the value of the binding with that
name, so `<List direction spacing children />` is `direction={direction}
spacing={spacing} children={children}`. There is no boolean shorthand: `visible`
means `visible={visible}`, never `visible={true}`.

A prop typed `String`, `Option<String>`, or `Prop<String>` accepts a string
literal directly, so `content="Inspector"` needs no `to_string()`.

An attribute whose name starts with `@` is a modifier rather than a prop:
`<Button @decorate=value />` builds the button, then calls `decorate(button,
value)`. The function is resolved in the caller's scope, and can be qualified
(`@my_module::decorate=value`). Modifiers run in source order after all props
and children have been built; repeated modifiers compose, and each may change
the value's type. Modifier values are evaluated after building too: compute one
before `view!` if its expression needs data that a child closure will move.
Expression and string children accept the same modifiers,
such as `{content.call(())} @sizing=ItemSize::Percent(100.0)`.

The GUI modifiers `@test_id="toolbar.button"`, `@node_ref=&panel`, and `@sizing`
respectively name the node for `find_test_id`, fill a `NodeRef`, and place the
node with its sizing among siblings. They are imported from `beui::reactive`,
usually through `use beui::reactive::*`. If a local binding shadows a modifier,
qualify the function path. Ordinary `test_id` and `node_ref` props are allowed
when the component declares them.

A tag whose required props are missing is a compile error at the tag that wrote
it, not a panic from inside the generated builder, and so is a prop written
twice on one tag. Every prop is required unless its
type or its attributes say otherwise: `Prop<T>` is as required as a plain `T`,
and a component that wants a prop to be optional says so with
`#[prop(default = <expr>)]`, or takes `Option<Prop<T>>` when it needs to tell
"unset" apart from any value the caller could have passed. `Callback`,
`ClickCallback`, and `Children` props keep their empty default, since a callback
nobody listens to and a tag with no children are ordinary states rather than
omissions. A `Binding<T>` prop is never required either: left unset, the
component creates its own signal, holding the `#[prop(default = ...)]` value or
else `T::default()`.

```rust
#[component]
fn Badge(
    label: Prop<String>,                                  // required
    #[prop(default = Color32::WHITE)] color: Prop<Color32>, // optional, with a default
    #[prop(default = None)] width: Prop<Option<f32>>,     // optional, and can clear itself
    tooltip: Option<Prop<String>>,                        // optional, absence is visible
) -> NodeId
```

An `Option<T>` prop takes a plain `T`, and also an `Option<T>`, so a wrapper
hands its own optional prop straight on with `action={action}`. Two kinds of `T`
take only the plain value: a primitive such as `f32`, so a numeric literal keeps
its type, and `String`, which takes anything that is `Into<String>`.

`Option<Prop<T>>` settles at build time: the tag either wrote the attribute or
it did not, and a signal behind it can only ever hand over another `T`. A prop
that has to go back to "nothing" while it is alive — a `Frame` whose `width`
stops constraining it and leaves it measuring intrinsically again — is
`Prop<Option<T>>` with `#[prop(default = None)]` instead. Its attribute takes a
plain `T` or a signal of one and lifts it into `Some`, as well as an
`Option<T>` or a signal of one written out in full, so `width=TRIGGER_WIDTH`
and `width={maybe_width}` are both that same prop.

The base elements and the structural primitives that insert and remove nodes for
a living — `show`, `dynamic`, `keyed`, `for_each`, `VirtualList` — are the only code that
touches `Document` directly. Everything above them, unstyled and styled
components and the code that uses them, says what it wants through props on
those tags.

Derive a prop that depends on other signals with `create_memo`, and pass the
memo straight to the prop; a memo only wakes the property when its value
actually changes.

```rust
let fill = create_memo(move || if hovered.get() { ColorRole::Hover } else { ColorRole::Surface });
view! { <Frame fill radius=Radius::Md>{child}</Frame> }
```

`Prop<T>` reads through `Get` like a signal: `get()` subscribes the
computation around it, and `get_untracked()` does not. A component that has to *read* one of its own
props wraps that in a memo it can read and pass on:

```rust
let text = create_memo(move || content.get());
```

### Bindings: state a component reads and writes

When the component also writes that value itself — a toggle whose `checked`
flips when clicked, a slider's `value`, a popover's `open` — the prop is a
`Binding<T>` rather than a `Prop<T>` plus an `on_change` callback. A binding
reads through `Get`, tracking like a signal, and writes through `Set`. The
component writes to it and reads it back; it never keeps a copy in sync with an
effect.

```rust
#[component]
pub fn Toggle(checked: Binding<bool>, /* ... */) -> NodeId {
    let flip = move || checked.set(!checked.get_untracked());
    let toggled = create_memo(move || Toggled::from(checked.get()));
    // ...
}
```

The caller decides who owns the state, with nothing to choose between on the
component's side:

- Leave the prop out and the component owns it: the macro creates a signal in
  the component's scope, seeded from `#[prop(default = ...)]`. This is what
  "uncontrolled" means here, and it is the default.
- Pass a `Signal<T>` and the component reads and writes that signal:
  `<Checkbox checked={done} />`. The binding holds just the signal's handle, so
  this costs no allocation.
- Pass a plain value to seed a signal the component then owns:
  `<Switch on=true />`.
- Pass `Binding::new(read, write)` for state that is derived or lives
  elsewhere: `read` is anything `IntoProp<T>` (a memo, a `Prop`), and `write`
  anything `IntoCallback<T>`, called with the value the component wants. The
  component shows what `read` says, so a write the callback ignores changes
  nothing on screen, and `set` skips a write equal to the value read. A
  plain value as `read` seeds a signal the binding owns, which each write
  updates before calling `write`: an uncontrolled control that also reports.

```rust
let selected = create_memo(move || state.tool.get() == tool);
view! { <ToggleButton pressed={Binding::new(selected, move |_| state.tool.set(tool))} /> }
```

To react to a change, derive from the signal (a memo, or an effect for side
effects outside the graph) instead of asking the component for a callback.
Callbacks that only report what happened (`on_hover_change`, `on_focus_change`,
`on_drag_change`, `on_preview`) stay callbacks, and take a signal as well as a
closure: `on_hover_change={hovered}` rather than
`on_hover_change={move |next| hovered.set(next)}`.

A component hands its binding to the content it renders as it is: a handle
field typed `Binding<T>` reads the state and could write it too. A binding is
also `IntoProp<T>` and `IntoCallback<T>`, so it passes straight to a prop or
callback of an inner component.

A component's child arity is part of its signature. A prop named `children`
typed `Children` takes however many are written between its tags; typed `Child`
it takes exactly one and arrives as the `NodeId` itself, so wrappers use
`{children}` in their `view!` without unwrapping, and a caller who writes none,
or more than one, does not compile. `Option<Child>` is the same for a wrapper
whose child is optional, like `fill` or a `button` that takes `content` instead:
none or one compiles and two do not, rather than the extras being dropped.
Inside a list, `{child}` may also be an `Option<NodeId>`, with or without
`@sizing`: `None` adds nothing. That is how a handle's optional part, such as a
tab bar a pane may not have, is placed without a `Show` around an `unwrap`.
A `Children` value cannot be handed to a `Child` or `Option<Child>` prop, since
its arity is only known once it is built; a wrapper that forwards its children
into a single-child slot declares that arity itself.

A `view!` with more than one root is a `Children` rather than a `NodeId`, so a
fixed set of siblings can be written in one place and handed to a `Children`
prop somewhere else. The number of roots decides, and nothing else: one root is
the `NodeId` it has always been. A `Children` slot takes that lone child as a
run of one, so a set written in one place reads the same whether it has one
member or several.

A `Children<T>` holds whatever kind of child its slot takes, and that kind need
not be a node: `unstyled::MenuItem` and `unstyled::ChoiceOption` are components
that build a value rather than a node, so a menu and a tab bar are written as
tags and each row follows the signals its tag was given. Declare such a type
with `value_child_type!`, which says how a run of it is kept, and it can be
written between tags or built by a `show` or a `for_each` like any other
child.

```rust
<ContextMenu items={view! {
    <MenuItem label="Copy" />
    <Show condition={pasteable}><MenuItem label="Paste" /></Show>
}} on_select={choose}>
    <Card>…</Card>
</ContextMenu>
```

A component reads such a slot with `into_run`, which answers with a `Run<T>`:
the children written into it now, and the ones a `show`, a `dynamic` or a
`for_each` inside it builds later. `Run::items` reads them and subscribes, so a
run that changes wakes whoever read it; `get_untracked` reads without subscribing, and
`build` is the usual consumer, turning the run's items into a run of children
of its own and keeping the two in step.

```rust
#[component]
fn Legend(entries: Children<Note>) -> NodeId {
    let entries = entries.into_run();
    let rows = entries.build(|notes: Vec<Rc<Note>>| {
        notes.iter().map(|note| view! { <Row note /> }).collect()
    });
    view! { <Column spacing=0.0 children={rows} /> }
}
```

A `Run<T>` is cheap to clone and fills a `Children<T>` slot again, which is how
one set of options reaches two controls: `styled::ResponsiveTabs` hands the same
run to its tabs and to the select it collapses into.

```rust
let toolbar = view! {
    <Button label="Open" on_click={open} />
    <Button label="Save" on_click={save} />
};
view! { <List direction=Direction::Horizontal spacing=8.0 children={toolbar} /> }
```

Roots take the same `@` sizing prefixes and `{expr}` form that children between
tags take, and `view! {}` is the empty `Children`. A sizing prefix on a lone
root is an error rather than a one-item `Children`, because with no siblings
there is nothing to take a share of. Fragments are for siblings written out in
source; a list built from runtime data is a `ForEach` over its keys, because a
`Vec` of children does not fill a `Children<T>` slot.

Props that build part of the tree are typed `Render<H>` when the component calls
them once, `RenderFn<H>` when it may call them many times, and `Option<..>` when
they have a default. Their setters take a bare closure, so a component hands
part of its chrome to its caller the way JSX passes children as a function.

`#[prop(children)]` marks the one of them that the block between the tags fills,
so a caller writes that subtree where it reads as the body instead of as an
attribute. A prop named `children` already is that slot and needs no annotation;
a component has at most one, and a component with both keeps `children`.

```rust
#[component]
fn Checkbox(label: Prop<String>, checked: Prop<bool>) -> NodeId {
    view! {
        <Toggle checked>|handle|
            <CheckboxFace handle label />
        </Toggle>
    }
}
```

A slot that hands something over — `toggle`'s `content` above, `for_each`'s
`view`, `container`'s `content`, `dynamic`'s and `keyed`'s `view` — names the
value right after the opening tag, the way a closure names its argument:
`<Container>|size| <Card size /></Container>`. Everything up to the closing tag
is the closure's body, which `view!` builds each time the component calls it.
The value's type comes from the slot, and may be written out (`|key: Uuid|`);
the closure is always `move`. When the body needs statements before its tags,
it is a block instead, the only thing before the closing tag:
`<Container>|size| { let narrow = narrower_than(W); view! { <Card narrow /> } }</Container>`.

Tags written without `|value|` are a closure that ignores the value, so a
slot whose value is `()`, like `show`'s `then`, takes its tags as they are, and
so does a button whose face needs nothing from its handle. Either way `view!`
wraps them in the closure itself, so they stay unbuilt until the component asks
for them. Naming a value on a slot whose value is `()` is a compile error, and
its block form is `|| { ... }`. A `RenderFn` slot, like `Show`'s or `ForEach`'s,
may run that closure more than once, so the body clones what it hands on
rather than moving it: `<Show condition>|| { let a = a.clone(); view! { ... } }</Show>`.
A closure written in braces between the tags, `{move |x| ...}`, is a compile
error pointing at this form.

```rust
<Show condition={tab.memo(0)}><ListControls rows={list_rows.clone()} /></Show>
```

Either way the block is exactly one child, because a `Render` returns one of
them; two tags there are a compile error naming the tag that wrote them. A slot
typed `Render<H, Children<T>>` or `RenderFn<H, Children<T>>` returns a run
instead, so it takes any number of tags, and the component hands what its call
returns to a `Children<T>` prop of its own.
A slot the component always calls is a required prop like any other, so a
`Show` with nothing between its tags, or a `ForEach` with no closure, does not
compile rather than panicking once the view runs. Each misuse the types turn
away like this has a case in `compile_fail/beui/` naming the error it gets.
Handing a slot a `Render`/`RenderFn` a component was given itself stays an
attribute, like `panel={panel.clone()}` — only a closure or a tag block can be
written between the tags.

A render prop runs with the component that *wrote* it installed, not the one
that calls it, so component-scoped state and accessibility inside one belong to
the outer component. `Func<V, R>` is the same idea for a plain callback that
returns a value, like `keyed`'s `key`. A prop of any of these three types also
accepts an already built `Render`/`RenderFn`/`Func`, which is how a component
forwards one it was given.

State a component keeps for its own handlers belongs in an `Rc` the handlers
capture. What others may read of it is a typed handle the component fills
through its `api` prop (`ComponentRef`, guides/beui.md), which a test holding
only the component's `NodeId` reads back with `Document::api`. Inside the tree,
reaching for a `NodeId` to find state again is a sign the value should have been
captured or passed as a prop.

## Controlled state

Anything a component would otherwise poke into a node after the fact is a prop
on the base element instead, so the component owns a signal and the node follows
it. `overlay` takes `open` and `anchor`, a focusable `interactive` takes `focused`,
`text_input` takes `value`, and `scroll` takes `offset` and `reveal` (the index
of the child to bring into view). `unstyled::button`, `unstyled::toggle` and
`unstyled::text_input` forward `focused` to the `interactive` underneath them.

These props are edge triggered: the effect behind them runs when the value it
reads changes, so a component that wants to move focus, or close a popup, writes
its signal and lets the effect do the work. Because the document can also change
that state on its own — a click moves focus, a scrim click dismisses an overlay —
pair the prop with the matching callback and write the signal back:
`on_focus_change` for `focused` and `on_dismiss` for `open`. Without the write
back the signal goes stale and the next write of the value it already holds
changes nothing. The document reports every way a node loses the focus,
including being hidden or removed, and a `focused` that starts `false` does not
touch the focus. `on_dismiss` reports only the document closing the overlay,
never the owner writing `open` false, so it can mean "cancel" without guarding
against the owner's own close.

A `Selector` is the natural source for `focused` in a list: keep one signal
naming the row that should have focus and give each row `focused={selection
.memo(Row(index))}`, so moving focus wakes only the row that lost it and the one
that gained it.

```rust
<unstyled::Button
    focused={focus.memo(Focus::Row(index))}
    on_focus_change={move |has_focus: bool| {
        if !has_focus && state.focus.get_untracked() == Focus::Row(index) {
            state.focus.set(Focus::Away);
        }
    }}
/>
```

When the shape of a subtree depends on a value rather than a flag, `dynamic`
rebuilds it: it holds one child, and every time its `value` changes it builds a
replacement from `view` and removes the old one. Use it where a `show` would
need the value itself rather than a boolean, like a panel whose fields come from
the kind of thing it is inspecting.

```rust
<Dynamic value={shape}>|shape: Shape|
    <ShapeFields shape />
</Dynamic>
```

`dynamic` rebuilds on every change of its value, which is right when the value
*is* the shape and wrong when only part of it decides the shape. `keyed` splits
the two: it rebuilds when its `key` changes and hands its `view` a
`ReadSignal<T>` for everything else, so the content of a branch updates in place
while the branch itself stays put.

```rust
<Keyed value={state} key={|state: State| state.shape()}>|state: ReadSignal<State>|
    <Screen state />
</Keyed>
```

None of `show`, `dynamic`, `keyed` or `for_each` builds a node. Each keeps a
run of children — none, one, or many — in a slot of the parent it is written
in, so its children are laid out by that parent and the children written around
it keep their places however the run changes. A hidden `show` is a slot with
nothing in it, so its siblings take the room because nothing is there.

The kind of child a run builds is the kind its parent takes: `ListChild`s in a
list, plain nodes in a `scroll`, `CanvasItem`s in a `canvas`. So a row says how
much room it wants the way any child of a list does — `@sizing` on the root it
returns — and a row builder that returns a plain node is intrinsic. Each row is
free to differ from the others and to change its mind reactively.

Because they build no node, `@test_id` and `@node_ref` on one of them are a
compile error. Each is a `DynamicSegment<C>`, which fills a slot that keeps a
run of children but not one that takes exactly one — `frame`'s child, a `render` prop — so
writing one there is a compile error, and a `List` around it is the fix. A
component whose view is one of them returns it as it is, typed
`-> DynamicSegment<C>`, and is written wherever a run of `C` fits.

A row builder returns whatever kind of child the parent takes, so a closure
that returns a plain node fits a list, a `scroll` or anything else that takes
one child per row without saying so.

```rust
<ForEach spacing=0.0 keys>
    |key: Uuid| <ItemRow @sizing={row_size(key)} key />
</ForEach>
```

`@sizing` on a child inside `view!`, or on a root of a multi-root one, gives it
an `ItemSize` in the `List` that lays it out, and a child without one is
`ItemSize::Intrinsic`. The value is `impl IntoProp<ItemSize>`, so it takes a
plain `ItemSize::Fixed(HEIGHT)` or `ItemSize::Percent(100.0)` as well as a
signal or memo of one, and a child can switch between kinds reactively — a memo
that reads `narrower_than` and returns `ItemSize::Fixed` in a column where it
returned `ItemSize::Percent` in a row, for instance.

```rust
<Toolbar @sizing=ItemSize::Fixed(TOOLBAR_HEIGHT) />
<Text @sizing=ItemSize::Percent(100.0) style=TextStyle::CAPTION align=TextAlign::End>{count_text}</Text>
<Card @sizing={rows_size}>
```

An `{expr}` child takes `@sizing` after it rather than inside the braces, which
is how a node handed to a component — a render prop's result, a `NodeId` a
caller built — takes a share of a list without a `Frame` wrapped around it:

```rust
<List direction=Direction::Horizontal spacing=SPACING>
    <Spacer @sizing={indent} />
    {content.call(key)} @sizing=ItemSize::Percent(100.0)
</List>
```

`@sizing` on the single root of a `view!` makes that `view!` build a `ListChild`
rather than a `NodeId`, which is what the row builder of a `Dynamic`, `ForEach`
or `Keyed` returns, so a row picks its own size the same way any other child
does. A percent child takes its share of
what is left over, so it needs a bounded main axis: inside a list that is being
measured intrinsically there is no leftover space to share, and percent children
fall back to their intrinsic length there, the way `height: 50%` of an
auto-height parent does in CSS. A `scroll` measures as nothing, so a percent
scroll inside an intrinsically measured column collapses; give it a fixed length
for that case. See
`crates/beui-demo` for a full example and
`crates/beui/src/tests/a_reactive_tree_can_nest_builder_calls_without_threading_the_document.rs`
and `.../a_signal_write_from_a_click_handler_updates_its_bound_text_in_the_same_frame.rs`
for the behavior they rely on.

## Context

`provide_context(value)` stores a value on the current scope, keyed by its type,
and `use_context::<T>()` walks up the owner chain and returns the nearest one, or
`None`. Because component scopes form the same tree the nodes do, a component
reads whatever its ancestors provided, and the effects it creates later — a
`show` branch that builds long after the first frame, for instance — see the same
values, since a computation's execution scope is parented to the scope that
created it. A scope holds on to the scope it was opened in, so a row that a
keyed list or `Show` keeps across its effect's reruns still reads what was
provided above it, even though the execution scope it was built in is gone. Provide a distinct wrapper type per concern rather than a bare `f32`,
or two providers will collide on the same key.

One ordering rule matters: `view!` builds a tag's children before the tag itself,
so a subtree passed as `Children` or `Child` is already built *before* the body
that would provide to it runs. A provider therefore takes the subtree it covers
as a render prop and calls it after `provide_context`. That prop is where
`#[prop(children)]` earns its keep: the caller still writes the subtree between
the provider's tags, but `view!` hands it over as a closure rather than as
finished nodes, so the provide-then-build order holds while the provider reads
like any other wrapper. Typed `Render<(), Children<T>>`, it takes as many
children as a `Children<T>` prop would:

```rust
#[component]
fn Page(#[prop(children)] sections: Render<(), Children<ListChild>>) -> NodeId {
    view! {
        <Container>|_size|
            <List spacing=0.0 children={sections.call(())} />
        </Container>
    }
}
```

## Containers and responsive layout

Content that depends on the space it is given is built inside layout, not
after it: a `LayoutBuilder` hands its render prop a `ReadSignal<Vec2>` of the
size it is laid out at, and rebuilds its content in the same walk when that
size changes (see "One layout per frame" in the beui guide). `node_rect(id)`
watches where a node was placed, for content positioned in document
coordinates; content that needs a rect to lay itself out uses a
`LayoutBuilder` instead.

`unstyled::container` ties the two together: it is a `LayoutBuilder`, which
builds its content in a scope of its own under the caller's, the way `Show`
and `Dynamic` do, so context the caller provides still reaches it. It provides
the size it is laid out at as `ContainerSize`, and hands the signal to its `content`
render prop. Anything below it can then ask `container_size()`, or
`narrower_than(width)` for a `Memo<bool>` that is true when the nearest container
is narrower than `width` (false until the first layout). Queries answer for the
nearest container, so a card that wraps its contents in a container gets answers
about the card, not the window. In debug builds `narrower_than` and `shorter_than`
panic when no container is above them (release builds answer false), which usually means children built in the caller's
scope rather than inside a wrapper's container (see the ordering rule above);
`container_size()` returns `None` there instead, for code that has a use for
"no container".

`unstyled::stack` consumes that flag: it is a row that turns into a column, with
its children falling back to intrinsic sizing, while the flag is true. It keeps
the same nodes across the switch, so state inside them survives.
`styled::stack` supplies `theme::NARROW_WIDTH` as the default breakpoint, and
`styled::responsive_tabs` swaps a tab bar for a select below one.

Measuring a node whose size depends on its own content — an intrinsically sized
container whose children react to its width — can oscillate. Give containers a
width that comes from their parent.

Each `Document` owns a root `reactive::Scope` (`Document::reactive_scope`).
Every node belongs to the scope that was current when it was created, just as an
effect does, and disposing that scope removes the nodes it still owns: a scope's
nodes, effects and `on_cleanup` callbacks go together. Every `#[component]`
runs in a `child_scope` of the scope that built it, so a component lives
exactly as long as its caller. Code built directly
in `build`'s closure belongs to the document's root scope and lives as long as
the document.

`Show`, `Dynamic`, `Keyed`, `ForEach` and `VirtualList` build each child in a
scope of its own and drop that scope to drop the child: `Show` disposes its
child's scope when it hides it, where `ShowKeepAlive` pauses it, and
`VirtualList` keeps a released row's scope to hand the row its next key. Anything else
that drops part of a view on its own keeps a `Scope` for it and drops that,
as the dock does for each tab's panel; nothing removes nodes by hand. A
component that returns a value rather than a node lives as long as the scope
that built it, like one that returns a node; dropping the value does nothing.

The scope that owns a node and the node that holds it can differ. When a node is
removed, its children that belong to another, still-live scope are detached
rather than removed, and live on until their own scope goes, which is what lets
a `Portal` show a subtree built elsewhere. The other way round, a node whose
scope is disposed while a parent still holds it is taken out of that parent
when it is removed: every `Element` says how to let go of a child with
`remove_child`.

An effect that writes one particular node belongs with that node rather than
with whichever scope is running: `bind(node, || ...)` creates it in the scope
that owns the node, so it stops when the node goes, wherever it was written.
A `NodeRef` lets go of its node when the node is removed, so an effect that
reaches a node through `try_get` finds nothing rather than a removed id.

None of these functions hold `&mut Document` across a call boundary: each reads
it back out of a thread-local (`with_document`) installed by whichever ambient
context is active, and releases it before returning. `build` installs the
document (and enters its scope, so a `Prop` bound to a signal can create its
effect) for the duration of the tree-building closure. `Document::show` installs
itself for each event it dispatches and flushes the effects that event queued
before the next one, outside any walk, so a signal write from a click handler
is visible in the same frame and to the events after it. A write inside an
entry is seen by effects when the outermost entry ends, with one exception:
`ScopeContext::flush_owned` (and `Scope::flush_owned`) runs, there and then,
the queued effects owned by that scope and the scopes under it, and leaves
every other effect queued. `KeyedItems::entry_owned` builds an entry and
flushes its scope that way. This is what a node uses to build its own subtree
during layout (see "One layout per frame" in the beui guide); nothing else
should need it.

Disposing a scope never removes its nodes on the spot. Each node's cleanup
queues its id with the document that made it, and the document removes them
once the code that disposed the scope has returned: after each event, after
layout, and when `with_reactive_scope` returns. A node therefore never leaves
the arena while something is walking it, and a scope disposed while another
document is installed still removes its nodes from its own.

`with_document` asserts when no document is installed at all; nested calls are
fine, and reach the same installed document. Event handlers are plain `FnMut`
closures with no document parameter, so they call ambient functions like any
other code. Keep those calls at the leaves: a component should read and write
node properties through props on base elements, and reach for `with_document`
only inside a base element or to look up its own component state. Effects always
run after the entry that queued them has released its borrow, so ambient calls
inside an effect body nest safely too.

# Reactive core

`crates/reactive` provides a dependency-free, single-threaded reactive graph. It
is intended for retained UI bindings: create a node once, then use an effect to
update its properties when the values it reads change. `beui::reactive` (in
`crates/beui-tree` and `crates/beui-view/src/reactive.rs`) is the adapter that
connects it to beui's components and `Document`; see "beui integration" below.

## Example

```rust
use reactive::{create_effect, create_memo, create_signal, Scope};

let scope = Scope::new();
let (count, set_count) = scope.run(|| create_signal(0));

scope.run(|| {
    let text = create_memo(move || format!("Count: {}", count.get()));
    create_effect(move || println!("{}", text.get()));
});

set_count.set(1);
set_count.update(|count| *count += 1);
```

Keep the scope alive for as long as the view exists. Dropping it or calling
`scope.dispose()` stops its computations, runs their cleanup callbacks, and
frees the signals and memos created in it.
Run `./scripts/buck run //crates/reactive:example-retained` for a complete example that
updates retained state and batches changes around a mutable borrow.

## Signals and memos

`create_signal(value)` returns `(ReadSignal<T>, WriteSignal<T>)`. Both handles
are `Copy`, including for values that do not implement `Clone`, so a `move`
closure takes its own copy and the original stays usable. A handle is a key into
a thread-local arena, not a reference count: the signal belongs to the scope
that was current when it was created and is freed when that scope is disposed.
Creating one with no current scope panics. Reading a freed signal panics;
`set` and `set_unconditionally` on one do nothing, since nothing can read the
value they would write, and `is_alive()` asks whether it is still there.
Handles and scopes cannot cross threads.

- `get()` clones a value and subscribes the current computation.
- `with(|value| ...)` borrows it and subscribes without requiring `Clone`.
- `get_untracked()` and `with_untracked(...)` read without subscribing.
- `set(value)` requires `PartialEq` and notifies only when the value changes.
- `update(|value| ...)` mutates in place, returns the closure's result, and always
  notifies. It works without `PartialEq` or `Clone`.
- `set_unconditionally(value)` replaces and always notifies.

`create_memo(|| ...)` creates a read-only cached computation with `PartialEq`
output. It computes once immediately, then refreshes on demand. A memo whose
first computation reads no signal and leaves nothing to clean up is a constant:
it keeps its value and no computation at all. Memos expose the
same reading methods as signals, are `Copy` the same way, and are freed with
their scope. A changed dependency invalidates downstream
computations, but equal memo outputs suppress downstream execution. Reading a
memo inside a batch returns its current value. Dependency chains and diamonds
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
  is freed when that value drops. `read()` and `write()` hand out `Copy`
  handles. Use it for a signal per entry in a map that outlives its readers
  (`KeyedStore` keeps one per key, and `Document` one per watched node), or for
  a struct field when the struct is made outside a scope.
- `RootScope::new()` makes a detached scope current until it is dropped, so
  everything created after it in the same block belongs to it. A test that
  creates signals before building a document starts with
  `let _root = RootScope::new();`. A constructor that creates many signals
  starts the same way and ends with `owner.into_scope()`, which stops it being
  current and returns the `Scope` for the struct to keep. Unlike `Scope::run`,
  it neither batches nor flushes. Drop or hand back a `RootScope` in the block
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

- `with(|value| ...)` borrows the value; `get()` returns its `Rc<T>`. Both panic
  once the scope is disposed. `try_with`, `try_get` and `is_alive` don't.
- It holds the value immutably; put a `Cell` or `RefCell` inside for state that
  changes, or use a signal when readers must hear about the change.
- `OwnedStored::new(value)` is the scope-free counterpart of `OwnedSignal`: it
  frees the value when it drops and hands out `Copy` handles with `handle()`
  (it also derefs to one). Use it for state made outside a scope, such as an
  editor the host keeps.

`Callback`, `ClickCallback` and `NodeRef` are `Copy` the same way. An empty
callback needs no scope; `Callback::new`, `ClickCallback::new` and
`NodeRef::new` create stored values, so they need one. Calling a callback
whose scope is gone does nothing, as `set` on a freed signal does.

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
let (selected, set_selected) = create_signal(Some(0usize));
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

Reads are current the moment they happen, including inside a batch that has not
flushed yet, and a key is forgotten once the last computation watching it is
disposed or stops reading it. Like memos, a selector must be created inside a
scope, and its per-key notifications stop when that scope is disposed.

## Stores

A signal holding a struct is one source for every field in it, so writing any
field wakes every computation that reads any other. `#[derive(Store)]` splits it:
for `Foo` it writes a `FooStore` whose fields are the `ReadSignal`s of `Foo`'s
fields, a `new(value)` that seeds them, a `set(value)` that writes each one (so a
field whose value did not change notifies nobody), a `set_<field>(value)` per
field, and a `get()` that reads the whole struct back. The store is `Copy`, and
its signals belong to the scope it was created in.

```rust
#[derive(Clone, PartialEq, Store)]
pub struct Theme {
    pub background: Color32,
    pub accent: Color32,
}

let theme = ThemeStore::new(Theme::DARK);
let accent: Prop<Color32> = theme.accent.into_prop();
theme.set_accent(Color32::WHITE);
```

Each field is a `ReadSignal<T>`, so it goes straight into a `Prop<T>`. A helper
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
    <ForEach keys={visible}>
        {move |id: Uuid| {
            let item = items.get(&id);
            view! { <ItemRow item /> }
        }}
    </ForEach>
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
inside the document's reactive scope, so an editor never pumps by hand. A
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
queues it for the host. It is already held behind whether the host says the
block may be edited, and dropped until the host has sent the content once, since
an edit made against content that has not arrived would land in the wrong place.
`read` answers the content outside a projection (`None` until the host has sent
it once), and `loaded()` is a signal that turns true when it has.

An editor shows nothing it would have to correct a moment later: not an empty
or editable view before its content arrives, and not a placeholder ("no rows")
that the next answer replaces. Every `BeuiApp::view` and `preview_view` wraps
its component in `<Loaded loaded={...}>`, which shows `Loading` (a spinner)
until the signal first turns true and only then builds what it wraps, and keeps
it from then on, so a later change updates the view in place instead of
bringing the spinner back. `editor.loaded::<C>(&[queries])` is the usual
signal: the editor's own content and the answer to each graph query it draws
names from, such as `BlockQuery::References(own)`. An editor that needs more
waits for that too - the version control editors for `editor.version_known()`,
the workspace shells for their view block, the database view for its database,
schema, relations and row labels - and one with a placeholder that is already
right while loading shows that instead (an image shows its thumbhash).
`RelatedContent::settled()` and `content_settled` turn true once a block's
content has arrived or the account cannot read it, so a block that will never
arrive does not leave an editor loading forever. Something that loads later,
inside a view that is already shown (a folder in the file tree being expanded),
says "Loading…" in its own place.

The graph around a block - its children, what it references, what references
it - is a query rather than content. `editor.watch_blocks(BlockQuery::Children(id))`
is a memo of the `BlockInfo`s the host answers that query with, `None` until the
first answer arrives, and it changes whenever the host's answer does.

## Effects, batches, and ownership

Create signals, memos, effects, and cleanup callbacks inside `Scope::run` or
another computation. `create_effect(|| ...)` schedules an initial execution, then runs
again when its tracked inputs change. Effects run synchronously before a write
returns, except inside `batch` or `Scope::run`, which flush when their outermost
batch finishes. Multiple writes coalesce into one pending execution. Effects
created inside effects run after their parent finishes.

An effect that finishes its first execution without reading a signal, opening a
scope, or registering a cleanup can never run again, so it is disposed right
there and releases whatever its closure captured.

`create_binding(|| ...)` is an effect whose first execution happens at once,
before the batch around it ends, the way a memo's does. If that execution read
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
A batch that panics does not flush effects while unwinding; pending work runs at
the next successful outer batch or write. A panicking `update` invalidates its
value because it may already have mutated it. There is no transaction rollback.

## Zones

`enter_zone(zone)` enters a zone until the guard it returns is dropped. An
effect remembers the zone that was entered when it was created, and the effects
it creates inherit it. While a different zone is entered, an effect woken by a
write is parked instead of run, and it runs when its own zone is next entered;
with no zone entered, every effect runs as it always did. `zone_pending(zone)`
says whether a zone has parked effects, and `forget_zone(zone)` drops them.
beui gives each `Document` a zone of its own and enters it whenever the
document is installed, which is what lets two documents share signals: a write
made while one document is installed does not rebuild another document's nodes
inside it, and the other document catches up the next time it is shown. A
host driving several documents should give the others a frame when
`zone_pending` says one of them is behind.

## Pausing

`Scope::detached().pausable()` (or `Scope::new().pausable()`, before anything
is created in it) is a scope that can be paused. While `pause()` holds, an
effect created under it that is woken waits instead of running, and
`resume()` runs each one that waited once, with the values as they are by
then. Memos are not paused, but they only compute when read, so a memo only
paused effects read waits with them. An effect already woken when the scope
pauses still runs, so the write that hides a subtree reaches it: an overlay
inside closes rather than staying open. A scope inside a paused scope waits
for it, whether or not it is paused itself. This is what keeps a hidden
`ShowKeepAlive` child from doing work nobody can see.

## beui integration

`beui::reactive` (re-exporting `create_signal`, `create_effect`, `create_binding`, `create_memo`,
`create_selector`, `store`, `Scope`, `batch`, `untrack`, `on_cleanup`,
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
    let (count, set_count) = create_signal(0i64);
    let count_text = create_memo(move || count.get().to_string());
    view! {
        <List spacing=0.0>
            <List direction=Direction::Horizontal spacing=8.0>
                <Button label="-" on_click={move || set_count.update(|count| *count -= 1)} />
                <Text string={count_text} /> // updates itself when `count` changes
                <Button label="+" on_click={move || set_count.update(|count| *count += 1)} />
            </List>
        </List>
    }
}

let document = build(|| view! { <App /> });
```

`#[component]` turns a function named `Name` into a `<Name>` usable from `view!`.
That exact function name is also the constructor that returns the builder `view!`
fills in. A prop typed `Prop<T>` accepts
either a plain `T` or a signal or memo of `T`; the base components bind each
prop with `create_binding`, which keeps a reactive form in sync and costs a
plain value nothing after it is set. Components return their base node
directly. Every tag also accepts the framework slots `@test_id`, `@node_ref`,
and `@sizing`, which live in their own `@` namespace so a component is free to
name its own props whatever it likes.

## Writing attributes

An attribute value in braces is any expression at all, and that is what closures,
blocks, method chains, and anything containing `<` or `>` need. Values simple
enough to read on their own may drop the braces: literals, paths, a call on a
path, a unary expression, and a reference to a path.

```rust
<Frame fill=ColorRole::Surface>
<Text style=TextStyle::CAPTION string=count_text align=TextAlign::End />
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

An attribute whose name starts with `@` is a framework slot rather than a prop
of the component: `@test_id="toolbar.button"` names the node for
`find_test_id`, `@node_ref=&panel` fills a `NodeRef` with the node once it is
built, and `@sizing` gives the node its `ItemSize` among its siblings. Writing
`test_id=` or `node_ref=` without the `@`, or an `@name` that is not one of
those three, is a compile error from `view!`.

A tag whose required props are missing is a compile error at the tag that wrote
it, not a panic from inside the generated builder, and so is a prop written
twice on one tag. Every prop is required unless its
type or its attributes say otherwise: `Prop<T>` is as required as a plain `T`,
and a component that wants a prop to be optional says so with
`#[prop(default = <expr>)]`, or takes `Option<Prop<T>>` when it needs to tell
"unset" apart from any value the caller could have passed. `Callback`,
`ClickCallback`, and `Children` props keep their empty default, since a callback
nobody listens to and a tag with no children are ordinary states rather than
omissions.

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

`Prop<T>` reads with `get()`, which subscribes the computation around it, and
with `peek()`, which does not. A component that has to *read* one of its own
props wraps that in a memo it can read and pass on:

```rust
let text = create_memo(move || content.get());
```

When the component also writes that value itself — a toggle whose `checked` prop
seeds state that clicking then changes — seed a signal from `peek()` and keep it
in sync with an effect, so writes from the component's own handlers and writes
from the caller's signal both land in one place. `Prop::map` adjusts the value on
the way in:

```rust
let value = value.map(|value| value.clamp(0.0, 1.0));
let (value_read, set_value) = create_signal(value.peek());
create_effect(move || set_value.set(value.get()));
```

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
run that changes wakes whoever read it; `peek` reads without subscribing, and
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
        <Toggle checked>
            {move |handle| view! { <CheckboxFace handle label /> }}
        </Toggle>
    }
}
```

A slot that hands something over — `toggle`'s `content` above, `for_each`'s
`view`, `container`'s `content` — takes one closure as its only child, and that
closure receives the handle. A slot whose handle is `()`, like `show`'s `then`,
takes tags instead: `view!` wraps them in the closure itself, so they stay
unbuilt until the component asks for them. A `RenderFn` slot, like `Show`'s,
may run that closure more than once, so the block clones what it hands on
rather than moving it: `{move || { let a = a.clone(); view! { ... } }}`.

```rust
<Show condition={tab.memo(0)}><ListControls rows={list_rows.clone()} /></Show>
```

Either way the block is exactly one child, because a `Render` returns one of
them; two tags there are a compile error naming the tag that wrote them.
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
capture; `set_component_state` additionally publishes it so that a test holding
the component's `NodeId` can read it back with `Document::component_state`. That
is the only reader: inside the tree, reaching for a `NodeId` to find state again
is a sign the value should have been captured or passed as a prop.

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
            state.set_focus.set(Focus::Away);
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
<Dynamic value={shape}>{move |shape: Shape| view! { <ShapeFields shape /> }}</Dynamic>
```

`dynamic` rebuilds on every change of its value, which is right when the value
*is* the shape and wrong when only part of it decides the shape. `keyed` splits
the two: it rebuilds when its `key` changes and hands its `view` a
`ReadSignal<T>` for everything else, so the content of a branch updates in place
while the branch itself stays put.

```rust
<Keyed value={state} key={|state: State| state.shape()}>
    {|state: ReadSignal<State>| view! { <Screen state /> }}
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
    {move |key: Uuid| view! { <ItemRow @sizing={row_size(key)} key /> }}
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
<Text @sizing=ItemSize::Percent(100.0) style=TextStyle::CAPTION string=count_text align=TextAlign::End />
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
like any other wrapper.

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
is narrower than `width` (false when nothing provides a size, and false until the
first layout). Queries answer for the nearest container, so a card that wraps its
contents in a container gets answers about the card, not the window.

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
is visible in the same frame and to the events after it. A write inside a
batch is seen by effects when the outermost batch ends, with one exception:
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
run after the batch that queued them has released its borrow, so ambient calls
inside an effect body nest safely too.

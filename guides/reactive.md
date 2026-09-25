# Reactive core

`crates/reactive` provides a dependency-free, single-threaded reactive graph. It
is intended for retained UI bindings: create a node once, then use an effect to
update its properties when the values it reads change. `beui::reactive` (in
`crates/beui/src/reactive.rs`) is the adapter that connects it to beui's
`Document`; see "beui integration" below.

## Example

```rust
use reactive::{create_effect, create_memo, create_signal, Scope};

let scope = Scope::new();
let (count, set_count) = create_signal(0);

scope.run(|| {
    let text = create_memo(move || format!("Count: {}", count.get()));
    create_effect(move || println!("{}", text.get()));
});

set_count.set(1);
set_count.update(|count| *count += 1);
```

Keep the scope alive for as long as the view exists. Dropping it or calling
`scope.dispose()` stops its computations and runs their cleanup callbacks.
Run `./scripts/buck run //crates/reactive:retained-example` for a complete example that
updates retained state and batches changes around a mutable borrow.

## Signals and memos

`create_signal(value)` returns `(ReadSignal<T>, WriteSignal<T>)`. Both handles
are cheaply cloneable, including for values that do not implement `Clone`.
Signals use reference-counted ownership and remain usable as long as a handle
exists, independently of scopes. Handles and scopes cannot cross threads.

- `get()` clones a value and subscribes the current computation.
- `with(|value| ...)` borrows it and subscribes without requiring `Clone`.
- `get_untracked()` and `with_untracked(...)` read without subscribing.
- `set(value)` requires `PartialEq` and notifies only when the value changes.
- `update(|value| ...)` mutates in place, returns the closure's result, and always
  notifies. It works without `PartialEq` or `Clone`.
- `set_unconditionally(value)` replaces and always notifies.

`create_memo(|| ...)` creates a read-only cached computation with `PartialEq`
output. It computes once immediately, then refreshes on demand. Memos expose the
same reading methods as signals. A changed dependency invalidates downstream
computations, but equal memo outputs suppress downstream execution. Reading a
memo inside a batch returns its current value. Dependency chains and diamonds
refresh before effects observe them, preventing intermediate derived values.

Dependencies are discovered on each execution. Conditional branches unsubscribe
from inputs they no longer read. `untrack(|| ...)` disables subscription for its
closure while preserving the current cleanup scope. Memo computations must be
pure: writing a signal inside a memo panics, including inside `untrack`.

`with` holds a shared borrow for the closure; `update` holds a mutable borrow.
Do not access the same signal incompatibly from those closures.

## Cloning handles into closures

Signals, memos, selectors, and the `Rc` handles components keep are cheap to
clone, and a `move` closure that keeps one has to own its own clone. `clone!`
writes those clones for you: it takes the names to clone, an arrow, and the
expression they are in scope for.

```rust
let text = create_memo(clone!(count -> move || count.get().to_string()));
create_effect(clone!(count state -> move || state.show(count.get())));
```

`clone!(a b -> expr)` expands to `{ let a = a.clone(); let b = b.clone(); expr }`,
so the originals stay usable afterwards and the last closure that needs a handle
can still take it by move.

## Selectors

A list of N rows that each ask "am I the selected one?" through a memo wakes all
N of them every time the selection moves. `create_selector(|| ...)` turns that
into two: it keeps one subscriber list per key that has been asked about, and
when its source changes it notifies only the key that lost selection and the key
that gained it.

```rust
let (selected, set_selected) = create_signal(Some(0usize));
let selection = create_selector(clone!(selected -> move || selected.get()));

for index in 0..rows {
    let selection = selection.clone();
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
field, and a `get()` that reads the whole struct back.

```rust
#[derive(Clone, PartialEq, Store)]
pub struct Theme {
    pub background: Color32,
    pub accent: Color32,
}

let theme = ThemeStore::new(Theme::DARK);
view! { <Frame color={theme.background.clone()} /> }
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
leaves the document untouched no matter how much of its content did. It panics
if two items claim the same key.

## Blocks

A plugin editor reads its block's content through a `ContentProjection`
(`block_editor_plugin`). `editor.block_content::<C>()` is the one for the
editor's own block, `editor.content_of::<C>(block)` the one for another block it
follows; both are pumped by the plugin framework once at the top of every frame,
inside the document's reactive scope, so an editor never pumps by hand.

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
and `object::<T>(id)` for one object and everything inside it. The checklist's
rows each watch their own item and its list watches only its order, so ticking
one item runs one row. A watcher is dropped with the reactive scope that made it.

Keep a projection cheap and pure; it must not operate. Write through
`ContentProjection::operate`, which applies the operation to what the view
sees straight away, so a local edit is visible in the frame that made it, and
queues it for the host. It is already held behind whether the host says the
block may be edited. `read` answers the content outside a projection (`None`
until the host has sent it once), and `loaded()` is a signal that turns true
when it has.

The graph around a block - its children, what it references, what references
it - is a query rather than content. `editor.watch_blocks(BlockQuery::Children(id))`
is a memo of the `BlockInfo`s the host answers that query with, `None` until the
first answer arrives, and it changes whenever the host's answer does.

## Effects, batches, and ownership

Create memos, effects, and cleanup callbacks inside `Scope::run` or another
computation. `create_effect(|| ...)` schedules an initial execution, then runs
again when its tracked inputs change. Effects run synchronously before a write
returns, except inside `batch` or `Scope::run`, which flush when their outermost
batch finishes. Multiple writes coalesce into one pending execution. Effects
created inside effects run after their parent finishes.

An effect that finishes its first execution without reading a signal, opening a
scope, or registering a cleanup can never run again, so it is disposed right
there and releases whatever its closure captured. This is what makes it cheap to
bind a property with an effect that may turn out to hold a plain value.

`create_effect` returns an `Effect` handle for explicit `dispose()` and
`is_disposed()` checks. Dropping this handle does not stop the effect; its scope
owns it. Effects may write signals, and resulting work is queued rather than
recursively executed. An effect exceeding 10,000 executions in one flush panics and is disposed
to stop runaway feedback loops.

A `Scope::new()` created inside an active scope becomes its child. Parent disposal
also disposes children, even when a child handle remains alive. A scope cannot
be entered after disposal; a memo cannot be read after its owning scope is
disposed. Disposal is idempotent.

Inside a computation, the current scope is that computation's execution scope,
which is thrown away every time it reruns. To open a scope that survives those
reruns but still belongs to the surrounding tree, use `owner_scope()`: it
returns the `ScopeContext` of the scope that created the running computation
(or the current scope when no computation is running). `ScopeContext::child()`
opens a child scope there, returning `None` if that scope is already gone, and
`ScopeContext::is_alive()` answers the same question on its own. This is how a
list keeps per-item scopes across updates of the list itself while still
tearing them down with whatever owns the list.

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

## beui integration

`beui::reactive` (re-exporting `create_signal`, `create_effect`, `create_memo`,
`create_selector`, `clone!`, `Scope`, `batch`, `settle`, `untrack`, `on_cleanup`,
`provide_context`, and `use_context` from this crate) binds signals and
memos directly to `Document` nodes. Nothing in a view takes a `&mut Document`
parameter, so tags nest the way JSX or solidjs would nest them: write the tree
with `view!` inside a `#[component]`, and build the document with `build`, which
supplies the document ambiently and returns the finished `Document`.

```rust
use beui::reactive::{
    build, component, create_memo, create_signal, view, Button, Direction, List,
    Text,
};

#[component]
fn App() -> beui::NodeId {
    let (count, set_count) = create_signal(0i64);
    let decrement = set_count.clone();
    let count_text = create_memo(move || count.get().to_string());
    view! {
        <List spacing=0.0>
            <List direction=Direction::Horizontal spacing=8.0>
                <Button on_click={move || decrement.update(|count| *count -= 1)}>
                    <Text string="-" />
                </Button>
                <Text string={count_text} /> // updates itself when `count` changes
                <Button on_click={move || set_count.update(|count| *count += 1)}>
                    <Text string="+" />
                </Button>
            </List>
        </List>
    }
}

let document = build(|| view! { <App /> });
```

`#[component]` turns a function named `Name` into a `<Name>` usable from `view!`.
That exact function name is also the constructor that returns the builder `view!`
fills in. A prop typed `Prop<T>` accepts
either a plain `T` or a signal or memo of `T`; the reactive forms create an
effect that keeps that property in sync. Components return their base node
directly. Every tag also accepts the framework slots `@test_id`, `@node_ref`,
and `@sizing`, which live in their own `@` namespace so a component is free to
name its own props whatever it likes.

## Writing attributes

An attribute value in braces is any expression at all, and that is what closures,
blocks, method chains, and anything containing `<` or `>` need. Values simple
enough to read on their own may drop the braces: literals, paths, a call on a
path, a unary expression, and a reference to a path.

```rust
<Frame color=SURFACE>
<Caption content=count_text align=TextAlign::End />
<List direction=Direction::Horizontal />
<Frame @node_ref=&panel width=TRIGGER_WIDTH height=HEIGHT>
<Frame color=Color32::from_gray(40) radius=4>
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
let fill = create_memo(move || if hovered.get() { HOVER } else { REST });
view! { <Frame color={fill} radius=RADIUS>{child}</Frame> }
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
create_effect(clone!(set_value -> move || set_value.set(value.get())));
```

A component's child arity is part of its signature. A prop named `children`
typed `Children` takes however many are written between its tags; typed `Child`
it takes exactly one and arrives as the `NodeId` itself, so wrappers use
`{children}` in their `view!` without unwrapping, and a caller who writes none,
or more than one, does not compile. `Option<Child>` is the same for a wrapper
whose child is optional, like `fill` or a `button` that takes `content` instead:
none or one compiles and two do not, rather than the extras being dropped.
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
unbuilt until the component asks for them.

```rust
<Show condition={tab.memo(0)}><ListControls rows=list_rows /></Show>
```

Either way the block is exactly one child, because a `Render` returns one of
them; two tags there are a compile error naming the tag that wrote them.
A slot the component always calls is a required prop like any other, so a
`Show` with nothing between its tags, or a `ForEach` with no closure, does not
compile rather than panicking once the view runs.
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
it. `overlay` takes `open` and `anchor`, `focusable` takes `focused`,
`text_input` takes `value`, and `scroll` takes `offset` and `reveal` (the index
of the child to bring into view). `unstyled::button`, `unstyled::toggle` and
`unstyled::text_input` forward `focused` to the `focusable` underneath them.

These props are edge triggered: the effect behind them runs when the value it
reads changes, so a component that wants to move focus, or close a popup, writes
its signal and lets the effect do the work. Because the document can also change
that state on its own — a click moves focus, a scrim click dismisses an overlay —
pair the prop with the matching callback and write the signal back:
`on_focus_change` for `focused` and `on_dismiss` for `open`. Without the write
back the signal goes stale and the next write of the value it already holds
changes nothing.

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

Because they build no node, `@test_id` and `@node_ref` on one of them panic,
and a slot that takes exactly one node — `frame`'s child, a `render` prop —
needs a `List` around one. A row builder returns whatever kind of child the
parent takes, so a closure that returns a plain node fits a list, a `scroll` or
anything else that takes one child per row without saying so.

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
<Caption @sizing=ItemSize::Percent(100.0) content=count_text align=TextAlign::End />
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
`crates/beui/examples/counter.rs` for a full example and
`crates/beui/src/document/tests/a_reactive_tree_can_nest_builder_calls_without_threading_the_document.rs`
and `.../a_signal_write_from_a_click_handler_updates_its_bound_text_in_the_same_frame.rs`
for the behavior they rely on.

## Context

`provide_context(value)` stores a value on the current scope, keyed by its type,
and `use_context::<T>()` walks up the owner chain and returns the nearest one, or
`None`. Because component scopes form the same tree the nodes do, a component
reads whatever its ancestors provided, and the effects it creates later — a
`show` branch that builds long after the first frame, for instance — see the same
values, since a computation's execution scope is parented to the scope that
created it. Provide a distinct wrapper type per concern rather than a bare `f32`,
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

Layout sizes are available reactively: `node_size(id)` returns a
`ReadSignal<Vec2>` that the document updates from that node's laid-out rect.
`Document::show` runs layout, publishes the sizes that changed, and lets the
effects that woke up rebuild before it lays out again — up to a few passes per
frame — so a size-driven change is visible in the frame that caused it rather
than one frame later. The signal reads `Vec2::ZERO` until the first layout.

`unstyled::container` ties the two together: it measures its returned base node,
provides that size as `ContainerSize`, and hands the signal to its `content`
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

Each `Document` owns a root `reactive::Scope` (`Document::reactive_scope`), and
every `#[component]` owns a scope of its own, registered against the base node
it returns. Several component scopes can share a node when wrapper components
return the same base node. A component's scope is a child of the scope that
built it. `Document::remove_node` disposes every scope registered against the
subtree it removes: the effects that were created while building those nodes
stop, and the `on_cleanup` callbacks they registered run. Without that, an
effect left alive by a removed component panics with "node was removed" the next
time one of its inputs changes.

Effects created outside any component body — directly in `build`'s closure, for
instance — belong to the document's root scope and live as long as the document.
`show`, `dynamic`, `keyed`, `for_each`, and `VirtualList` open a scope per child they
build, registered against that child's node, so dropping a row or scrolling one
out of view disposes exactly that row's effects. Any other code that builds a subtree
it will later remove on its own must do the same, with `in_new_scope`; building
it in the enclosing component's scope instead leaves the subtree's effects alive
after `remove_node` and they panic the next time an input changes.

None of these functions hold `&mut Document` across a call boundary: each reads
it back out of a thread-local (`with_document`) installed by whichever ambient
context is active, and releases it before returning. `build` installs the
document (and enters its scope, so a `Prop` bound to a signal can create its
effect) for the duration of the tree-building closure. `Document::show` installs
itself before dispatching interaction events and flushes queued effects
immediately after, before the frame's paint check, so a signal write from a
click handler is visible in the same frame.

`settle` runs a closure and then flushes the effects it queued even when an
outer batch is still open. `VirtualList` builds each row inside one, because it
measures the row immediately afterwards and the row's own props are applied by
effects; without it a row would measure as empty and the list would build every
item in the collection on its first frame.

`with_document` asserts when no document is installed at all; nested calls are
fine, and reach the same installed document. Event handlers are plain `FnMut`
closures with no document parameter, so they call ambient functions like any
other code. Keep those calls at the leaves: a component should read and write
node properties through props on base elements, and reach for `with_document`
only inside a base element or to look up its own component state. Effects always
run after the batch that queued them has released its borrow, so ambient calls
inside an effect body nest safely too.

# The reactive graph

How beui's state works: signals, memos, effects, scopes, stores, context, and how they bind to a
`Document`'s nodes. Read it when a component needs state, when something updates too often or
not at all, or when you need to know who owns a node or an effect. Writing views and layout is in
`guides/beui/views.md`; keys and focus are in `guides/beui/input.md`.

`crates/reactive` is a dependency-free, single-threaded reactive graph for retained UI bindings:
create a node once, then let an effect update its properties when what they read changes.
`beui::reactive` (`crates/beui-tree`, `crates/beui-view/src/reactive.rs`) is the adapter that
binds it to beui's components and `Document`, and re-exports `create_signal`, `create_effect`,
`create_binding`, `create_memo`, `create_selector`, `store`, `Scope`, `batch`, `untrack`,
`on_cleanup`, `provide_context` and `use_context`.

## A first example

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

Keep the scope alive as long as the view exists. Dropping it or calling `scope.dispose()` stops
its computations, runs their cleanups, and frees the signals and memos created in it.

`./scripts/buck run //crates/reactive:example-retained` is a complete example that updates
retained state and batches changes around a mutable borrow.

The same thing in beui, with nodes:

```rust
use beui::reactive::{
    build, component, create_memo, create_signal, view, Button, Direction, List, Text,
};

#[component]
fn App() -> beui::NodeId {
    let (count, set_count) = create_signal(0i64);
    let count_text = create_memo(move || count.get().to_string());
    view! {
        <List direction=Direction::Horizontal spacing=8.0>
            <Button on_click={move || set_count.update(|count| *count -= 1)}>
                <Text string="-" />
            </Button>
            <Text string={count_text} /> // updates itself when `count` changes
            <Button on_click={move || set_count.update(|count| *count += 1)}>
                <Text string="+" />
            </Button>
        </List>
    }
}

let document = build(|| view! { <App /> });
```

Nothing in a view takes a `&mut Document`, so tags nest the way JSX or SolidJS nest them. `build`
supplies the document ambiently and returns the finished `Document`.

## Signals

`create_signal(value)` returns `(ReadSignal<T>, WriteSignal<T>)`.

| Method | What it does |
|---|---|
| `get()` | Clones the value and subscribes the current computation. |
| `with(\|value\| ...)` | Borrows it and subscribes; no `Clone` needed. |
| `get_untracked()`, `with_untracked(...)` | Read without subscribing. |
| `set(value)` | Needs `PartialEq`; notifies only when the value changes. |
| `update(\|value\| ...)` | Mutates in place, returns the closure's result, always notifies. No `PartialEq` or `Clone` needed. |
| `set_unconditionally(value)` | Replaces and always notifies. |
| `is_alive()` | Whether the signal still exists. |

Handles and their lifetime:

- Both handles are `Copy`, even for values that are not `Clone`, so a `move` closure takes its own
  copy and the original stays usable.
- A handle is a key into a thread-local arena, not a reference count. The signal belongs to the
  scope current when it was created and is freed when that scope is disposed.
- Creating a signal with no current scope panics. Reading a freed signal panics; `set` and
  `set_unconditionally` on one do nothing, since nothing could read the value.
- Handles and scopes cannot cross threads.
- `with` holds a shared borrow for its closure and `update` a mutable one. Do not access the same
  signal incompatibly from inside those closures.

## Memos

`create_memo(|| ...)` is a read-only cached computation with `PartialEq` output. It exposes the
same reading methods as a signal, is `Copy` the same way, and is freed with its scope.

- It computes once immediately, then refreshes on demand when read.
- A changed dependency invalidates downstream computations, but an equal memo output suppresses
  downstream execution. Use a memo to derive a prop so the property wakes only when the derived
  value changes.
- Chains and diamonds refresh before effects observe them, so no effect sees an intermediate
  derived value. Reading a memo inside a batch returns its current value.
- A memo whose first computation reads no signal and leaves nothing to clean up is a constant: it
  keeps its value and no computation exists.
- Memos must be pure. Writing a signal inside one panics, even inside `untrack`, and
  `./scripts/verify` reports a `set`, `update` or `set_unconditionally` written inside a
  `create_memo` closure outside tests.

Dependencies are rediscovered on each execution, so a conditional branch unsubscribes from inputs
it no longer reads. `untrack(|| ...)` turns subscription off for its closure while keeping the
current cleanup scope.

## Effects and bindings

Create signals, memos, effects and cleanups inside `Scope::run` or another computation.

`create_effect(|| ...)` schedules a first execution, then reruns when its tracked inputs change.

- Effects run synchronously before a write returns, except inside `batch` or `Scope::run`, which
  flush when their outermost batch finishes. Several writes coalesce into one pending execution.
- Effects created inside an effect run after their parent finishes.
- Effects may write signals; the resulting work is queued, not run recursively.
- An effect that runs more than 10,000 times in one flush panics and is disposed, to stop runaway
  feedback loops.
- An effect whose first execution reads no signal, opens no scope and registers no cleanup can
  never run again, so it is disposed at once and releases what its closure captured.
- It returns an `Effect` handle with `dispose()` and `is_disposed()`. Dropping the handle does not
  stop the effect; its scope owns it.

`create_binding(|| ...)` is an effect whose first execution happens at once, before the batch
around it ends, like a memo's. If that execution read nothing and left nothing to clean up, no
computation is made; otherwise it reruns like an effect.

- Use it to bind a property of a node that already exists, as the base components do with their
  props. Most props are plain values, and binding one costs only its closure.
- Each binding still costs a probe, so bind props that are applied together in one binding, as
  `Text` binds its whole `TextStyle`. The setters skip unchanged values, so rerunning the group
  for one prop touches nothing else.
- Anything that must wait for the surrounding tree to be built, such as moving focus, stays an
  effect.

## Scopes and ownership

Every computation has a fresh execution scope: before it reruns, its old nested computations and
cleanups are disposed.

A scope has exactly one owner. There are two kinds:

- `child_scope(|| ...)` runs its closure in a new child of the current scope that lives exactly as
  long as that scope, and returns what it built with a `ScopeContext` to enter it again later.
  Every `#[component]` runs in one. It panics with no current scope, since nothing would dispose
  it.
- A `Scope` value owns its scope and disposes it when dropped.
  - `Scope::new()` inside an active scope becomes its child and is also disposed with it, even if
    the handle is still alive.
  - `Scope::detached()` opens one under the current scope, so it reads that scope's context and
    pauses with it, but only the value disposes it.
  - Keep the value where its lifetime belongs: in a `store` when it should go with the current
    scope (the control-flow components keep each branch's scope beside the child it built), or
    in a map entry when it goes with the entry, as `KeyedItems` and the dock's panels do.

Disposal is idempotent, a disposed scope cannot be entered again, and disposing frees the
signals, memos and selectors the scope owns.

A `ScopeContext` is a weak handle that owns nothing.

- Inside a computation, the current scope is its execution scope, thrown away on every rerun.
  `owner_scope()` returns the `ScopeContext` of the scope that created the running computation
  (or of the current scope when none is running).
- `ScopeContext::child()` opens a scope under it that the returned handle owns, or `None` if the
  scope is gone; `is_alive()` asks the same question.
- This is how a list keeps per-item scopes across updates of the list while still reading what
  was provided above it.

Cleanups:

- `on_cleanup(|| ...)` registers a callback on the current scope. Cleanups run in reverse
  registration order without tracking reads.
- Every cleanup is attempted even if one panics; the first panic propagates unless disposal is
  already unwinding.
- `on_release(target, key)` registers `target.release(key)` for a `Release` target shared by many
  registrations, so it allocates nothing. beui queues a node's removal this way for every node a
  scope owns.

Panics: tracking, ownership and scheduler state are restored when user code panics. A batch that
panics does not flush effects while unwinding; pending work runs at the next successful outer
batch or write. A panicking `update` invalidates its value, since it may already have mutated it.
There is no transaction rollback.

### State made outside a scope

State that outlives any one scope, or is made where no scope is current, owns its handles one of
these ways:

| Use | When |
|---|---|
| `OwnedSignal::new(value)` | A signal owned by the value holding it, freed when that drops. `read()` and `write()` hand out `Copy` handles. For a signal per entry in a long-lived map (`KeyedStore` keeps one per key, `Document` one per watched node), or a struct field when the struct is made outside a scope. |
| `OwnedStored::new(value)` | The same for a `Stored` value. `handle()` hands out `Copy` handles, and it derefs to one. For state such as an editor the host keeps. |
| `OwnedKeyedStore::new()` | The same for a `KeyedStore`; derefs to the handle. |
| `RootScope::new()` | Makes a detached scope current until dropped, so everything created after it in the block belongs to it. |
| `Scope::run` | Creates handles in an existing scope. |

`RootScope` details:

- A test that creates signals before building a document starts with
  `let _root = RootScope::new();`.
- A constructor that creates many signals starts the same way and ends with `owner.into_scope()`,
  which stops it being current and returns the `Scope` for the struct to keep.
- Unlike `Scope::run`, it neither batches nor flushes. Drop or hand back a `RootScope` in the
  block that made it.

A long-lived scope keeps every signal created in it until it is disposed. State that adds and
removes items over its life gives each item an `OwnedSignal` or a scope of its own instead.

## Sharing state between closures

Signals, memos, selectors, stored values, callbacks, node refs and structs made only of them are
`Copy`, so a `move` closure takes them without ceremony, and a closure capturing only `Copy`
values is itself `Copy`. Such a struct derives `Clone, Copy` and is passed by value.

`store(value)` puts any value in the signal arena and returns a `Stored<T>`, a `Copy` handle owned
by the current scope. Use it for state a component's closures share that is not itself reactive:
an editor's state struct, a `Cell` a gesture writes, a map of children.

- `with(|value| ...)` borrows the value; `get()` returns its `Rc<T>`. Both panic once the scope is
  disposed; `try_with`, `try_get` and `is_alive` do not.
- The value is held immutably. Put a `Cell` or `RefCell` inside for state that changes, or use a
  signal when readers must hear about the change.

`StoredCell::new(value)` is a `Copy` cell for a `Copy` value several closures read and write (the
point a gesture started at), with `get`, `set`, `replace`, `take` and `update`, owned by the scope
like a `Stored`.

`Callback`, `ClickCallback` and `NodeRef` are `Copy` the same way. An empty callback needs no
scope; `Callback::new`, `ClickCallback::new` and `NodeRef::new` create stored values, so they do.
Calling a callback whose scope is gone does nothing.

A closure that needs its own copy of an owned value (a `String`, a `Vec`, a `Prop<String>`) clones
it in a block around the closure. A value several closures share is better stored once with
`store` than cloned into each.

```rust
let accessibility = create_memo({
    let label = label.clone();
    move || named(label.get())
});
```

## Narrow subscriptions

### Selectors

A list of N rows that each ask "am I selected?" through a memo wakes all N whenever the selection
moves. `create_selector(|| ...)` keeps one subscriber list per key asked about and, when its
source changes, notifies only the key that lost selection and the key that gained it.

```rust
let (selected, set_selected) = create_signal(Some(0usize));
let selection = create_selector(move || selected.get());

for index in 0..rows {
    create_effect(move || highlight(index, selection.is_selected(&Some(index))));
}
```

- `is_selected(&key)` subscribes to that key alone, never to the source, so the computation
  reruns only when its own answer flips.
- `memo(key)` wraps one key in a `Memo<bool>` for props that want a value.
- Keys need `Clone + Eq + Hash`. The source's value is the key type, so an optional selection is a
  `Selector<Option<K>>` queried with `is_selected(&Some(key))`.
- Reads are current the moment they happen, even inside an unflushed batch. A key is forgotten
  once the last computation watching it is disposed or stops reading it.
- A selector must be created in a scope, and its notifications stop when that scope is disposed.

### Stores

A signal holding a struct is one source for every field, so writing any field wakes every reader
of any other. `#[derive(Store)]` on `Foo` generates a `Copy` `FooStore` whose signals belong to the
scope it was created in:

- fields that are the `ReadSignal`s of `Foo`'s fields;
- `new(value)`, which seeds them;
- `set(value)`, which writes each field, so an unchanged field notifies nobody;
- `set_<field>(value)` per field;
- `get()`, which reads the whole struct back.

```rust
#[derive(Clone, PartialEq, Store)]
pub struct Theme {
    pub background: Color32,
    pub accent: Color32,
}

let theme = ThemeStore::new(Theme::DARK);
view! { <Frame color={theme.background} /> }
theme.set_accent(Color32::WHITE);
```

Each field goes straight into a `Prop<T>`. A helper that picks between fields takes the store and
reads only the field its branch needs; since tracking is per execution, a memo around it
subscribes to that one field and resubscribes when the branch changes.

```rust
fn fill(theme: &ThemeStore, hovered: bool) -> Color32 {
    match hovered {
        true => theme.accent.get(),
        false => theme.background.get(),
    }
}
```

`beui::styled::use_theme` returns the document's `ThemeStore`, which is why a component binds a
colour rather than the theme.

### Keyed stores

`KeyedStore<K, V>` is the same idea for a list: one signal per item, plus a signal holding the
order. It is a `Copy` handle owned by the scope that made it (`OwnedKeyedStore` outside a scope).

```rust
let items: KeyedStore<Uuid, Item> = KeyedStore::new();
items.reconcile(list.iter().map(|item| (item.id, item)));
```

- `reconcile` takes the items as they are now, writes only the ones whose value differs, writes
  the order only when it differs, and forgets items whose keys are gone. It takes values by
  reference and clones only those it writes; `reconcile_owned` takes owned values.
- `keys()` is the order signal. Read it with `with` rather than `get` so the list is not cloned.
- `get(&key)` is that item's signal, the same handle across reconciles. A row binds to it, which
  is why `for_each` takes keys rather than values:

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

Key rows by identity, never by content. A key that contains the row's content (the whole item, or
its text) changes whenever the content does, which destroys the row's nodes and scope and builds
a replacement, losing focus, caret and measured text. An identity key survives the edit, and only
the bindings that read what changed run.

`for_each` reconciles in place, so a list whose order did not change leaves the document untouched
however much of its content did. A key that appears more than once gets a row per appearance,
matched in order: never wrong, only slower to edit.

## Components and props

`#[component]` turns a function `Name` into a `<Name>` tag for `view!`; the same name is the
constructor of the builder `view!` fills in. Components return their base node directly. The base
elements and the structural primitives that insert and remove nodes (`show`, `dynamic`, `keyed`,
`for_each`, `VirtualList`) are the only code that touches `Document`; everything above them says
what it wants through props on those tags.

### Reading and deriving props

A `Prop<T>` accepts a plain `T` or a signal or memo of `T`. The base components bind each prop
with `create_binding`, which keeps a reactive form in sync and costs a plain value nothing after it
is set.

- `get()` reads and subscribes the computation around it; `peek()` does not.
- Derive a prop from other signals with `create_memo` and pass the memo straight to the prop:

```rust
let fill = create_memo(move || if hovered.get() { HOVER } else { REST });
view! { <Frame color={fill} radius=RADIUS>{child}</Frame> }
```

- A component that must read one of its own props and pass it on wraps it in a memo:
  `let text = create_memo(move || content.get());`
- When the component also writes the value itself (a toggle whose `checked` prop seeds state that
  clicks then change), seed a signal from `peek()` and sync it with an effect, so the component's
  own writes and the caller's signal land in one place. `Prop::map` adjusts the value on the way
  in:

```rust
let value = value.map(|value| value.clamp(0.0, 1.0));
let (value_read, set_value) = create_signal(value.peek());
create_effect(move || set_value.set(value.get()));
```

### Required and optional props

Every prop is required unless its type or attributes say otherwise. A missing required prop, or a
prop written twice on one tag, is a compile error at the tag.

```rust
#[component]
fn Badge(
    label: Prop<String>,                                    // required
    #[prop(default = Color32::WHITE)] color: Prop<Color32>, // optional, with a default
    #[prop(default = None)] width: Prop<Option<f32>>,       // optional, and can clear itself
    tooltip: Option<Prop<String>>,                          // optional, absence is visible
) -> NodeId
```

| Type | Meaning |
|---|---|
| `Prop<T>` | Required, like a plain `T`. |
| `#[prop(default = <expr>)]` | Optional with a default. |
| `Option<Prop<T>>` | Optional, and "unset" is distinguishable. Settles at build time: a signal behind it can only hand over another `T`. |
| `Prop<Option<T>>` with `#[prop(default = None)]` | A prop that can go back to "nothing" while alive, like a `Frame` `width` that stops constraining. Takes a plain `T` or a signal of one (lifted into `Some`), or an `Option<T>` or signal of one written out in full: `width=TRIGGER_WIDTH` and `width={maybe_width}` are the same prop. |
| `Option<T>` | Takes a plain `T` or an `Option<T>`, so a wrapper forwards its own optional prop with `action={action}`. A primitive such as `f32` takes only the plain value (so a numeric literal keeps its type), and `String` takes anything `Into<String>`. |
| `Callback`, `ClickCallback`, `Children` | Default to empty: nobody listening and no children are ordinary states. |

A prop typed `String`, `Option<String>` or `Prop<String>` takes a string literal directly, so
`content="Inspector"` needs no `to_string()`.

### Writing attributes

An attribute value in braces is any expression; closures, blocks, method chains and anything
containing `<` or `>` need braces. Literals, paths, a call on a path, a unary expression and a
reference to a path may drop them:

```rust
<Frame color=SURFACE>
<Text style=TextStyle::CAPTION string=count_text align=TextAlign::End />
<Frame @node_ref=&panel width=TRIGGER_WIDTH height=HEIGHT>
<Frame color=Color32::from_gray(40) radius=4>
```

A bare name takes the binding of that name: `<List direction spacing children />` is
`direction={direction} spacing={spacing} children={children}`. There is no boolean shorthand:
`visible` means `visible={visible}`, never `visible={true}`.

Attributes starting with `@` are framework slots, not props, so a component may name its props
anything:

- `@test_id="toolbar.button"` names the node for `find_test_id`.
- `@node_ref=&panel` fills a `NodeRef` once the node is built; `NodeRef::get` reads it back. Use it
  when an enclosing component needs the `NodeId` of something nested in its tree.
- `@sizing` gives the node its `ItemSize` among its siblings (see "Control flow" below).

`test_id=` or `node_ref=` without the `@`, or any other `@name`, is a compile error.

### Children and arity

A component's child arity is part of its signature.

| Prop | Takes |
|---|---|
| `children: Children` | Any number of children written between its tags. |
| `children: Child` | Exactly one, arriving as the `NodeId` itself, so a wrapper writes `{children}` without unwrapping. None or two do not compile. |
| `children: Option<Child>` | None or one (for example `fill`, or a `button` that takes `content` instead); two do not compile. |

- Inside a list, `{child}` may be an `Option<NodeId>`, with or without `@sizing`; `None` adds
  nothing. That places an optional part of a handle (a tab bar a pane may not have) without a
  `Show` around an `unwrap`.
- A `Children` value cannot fill a `Child` or `Option<Child>` prop, since its arity is known only
  once built. A wrapper that forwards children into a single-child slot declares that arity
  itself.
- A `view!` with more than one root is a `Children`, not a `NodeId`, so a fixed set of siblings
  can be written in one place and handed to a `Children` prop elsewhere. One root is a `NodeId`,
  and a `Children` slot takes it as a run of one.
- Roots take the same `@` sizing prefixes and `{expr}` form as children between tags. `view! {}` is
  the empty `Children`. A sizing prefix on a lone root is an error.
- Fragments are for siblings written in source. A list built from runtime data is a `ForEach` over
  its keys, because a `Vec` of children does not fill a `Children<T>` slot.

```rust
let toolbar = view! {
    <Button label="Open" on_click={open} />
    <Button label="Save" on_click={save} />
};
view! { <List direction=Direction::Horizontal spacing=8.0 children={toolbar} /> }
```

### Value children

A `Children<T>` need not hold nodes. `unstyled::MenuItem` and `unstyled::ChoiceOption` are
components that build a value rather than a node, so a menu and a tab bar are written as tags and
each row follows the signals its tag was given. Declare such a type with `value_child_type!`,
which says how a run of it is kept; it can then be written between tags or built by a `show` or a
`for_each`.

```rust
<ContextMenu items={view! {
    <MenuItem label="Copy" />
    <Show condition={pasteable}><MenuItem label="Paste" /></Show>
}} on_select={choose}>
    <Card>…</Card>
</ContextMenu>
```

A component reads such a slot with `into_run`, which returns a `Run<T>`: the children written now
and those a `show`, `dynamic` or `for_each` inside it builds later.

- `Run::items` reads them and subscribes; `peek` does not.
- `build` turns the run's items into a run of children of its own and keeps the two in step.
- A `Run<T>` is cheap to clone and fills a `Children<T>` slot again, so one set of options can
  reach two controls: `styled::ResponsiveTabs` hands the same run to its tabs and to the select it
  collapses into.

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

### Render props

Props that build part of the tree let a component hand part of its chrome to its caller, the way
JSX passes children as a function. Their setters take a bare closure.

| Type | Use |
|---|---|
| `Render<H>` | The component calls it once. |
| `RenderFn<H>` | It may be called many times. |
| `Option<Render<H>>` etc. | It has a default. |
| `Func<V, R>` | A plain callback returning a value, like `keyed`'s `key`. |

Each of these also accepts an already built `Render`/`RenderFn`/`Func`, which is how a component
forwards one it was given; that stays an attribute, like `panel={panel.clone()}`.

`#[prop(children)]` marks the render prop that the block between the tags fills, so the caller
writes that subtree as the body. A prop named `children` already is that slot; a component has at
most one, and one with both keeps `children`.

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

- A slot that hands something over (`toggle`'s `content`, `for_each`'s `view`, `container`'s
  `content`) takes one closure as its only child, which receives the handle.
- A slot whose handle is `()`, like `show`'s `then`, takes tags instead; `view!` wraps them in the
  closure so they stay unbuilt until the component asks:
  `<Show condition={tab.memo(0)}><ListControls rows={list_rows.clone()} /></Show>`.
- The block is exactly one child; two tags there are a compile error naming the tag.
- A `RenderFn` slot may run its closure more than once, so clone what it hands on:
  `{move || { let a = a.clone(); view! { ... } }}`.
- A slot the component always calls is required, so a `Show` with nothing between its tags, or a
  `ForEach` with no closure, does not compile. Each misuse the types reject has a case in
  `compile_fail/beui/` naming its error.
- A render prop runs with the component that wrote it installed, not the one that calls it, so
  component-scoped state and accessibility inside it belong to the outer component.

### Component state

State a component keeps for its own handlers belongs in an `Rc` (or a `store`) the handlers
capture. `set_component_state` also publishes it so a test holding the component's `NodeId` can
read it with `Document::component_state`. That is the only reader: inside the tree, reaching for a
`NodeId` to find state again means the value should have been captured or passed as a prop.

## Control flow: show, dynamic, keyed, for_each

| Primitive | Rebuilds |
|---|---|
| `show` | Builds its child when the condition turns true, disposes it when false. `ShowKeepAlive` pauses it instead (see "Pausing"). |
| `dynamic` | On every change of its `value`, builds a replacement from `view` and removes the old one. |
| `keyed` | Only when its `key` changes; hands `view` a `ReadSignal<T>` for everything else, so the branch's content updates in place. |
| `for_each` | Keeps one child per key, reconciled in place (see "Keyed stores"). |

Use `dynamic` where a `show` would need the value rather than a boolean, such as a panel whose
fields come from the kind of thing it inspects. Use `keyed` when only part of the value decides
the shape:

```rust
<Dynamic value={shape}>{move |shape: Shape| view! { <ShapeFields shape /> }}</Dynamic>

<Keyed value={state} key={|state: State| state.shape()}>
    {|state: ReadSignal<State>| view! { <Screen state /> }}
</Keyed>
```

None of them builds a node. Each keeps a run of children (none, one or many) in a slot of the
parent it is written in, so the parent lays them out and the siblings around keep their places
however the run changes. A hidden `show` is an empty slot, so its siblings take the room.

- `@test_id` and `@node_ref` on one of them are a compile error.
- Each is a `DynamicSegment<C>`. It fills a slot that keeps a run of children, not one that takes
  exactly one (`frame`'s child, a `render` prop); put a `List` around it there.
- A component whose view is one of them returns it typed `-> DynamicSegment<C>` and is written
  wherever a run of `C` fits.

The kind of child a run builds is the kind its parent takes: `ListChild`s in a list, plain nodes
in a `scroll`, `CanvasItem`s in a `canvas`. A row builder that returns a plain node fits anything
that takes one child per row, and in a list it is intrinsic.

### Sizing children

`@sizing` on a child inside `view!`, or on a root of a multi-root one, gives it an `ItemSize` in
the `List` that lays it out; without it a child is `ItemSize::Intrinsic`. It takes
`impl IntoProp<ItemSize>`, so a plain value or a signal or memo, and a child can switch kinds
reactively (a memo that reads `narrower_than` and returns `Fixed` in a column and `Percent` in a
row).

```rust
<Toolbar @sizing=ItemSize::Fixed(TOOLBAR_HEIGHT) />
<Text @sizing=ItemSize::Percent(100.0) style=TextStyle::CAPTION string=count_text />
<Card @sizing={rows_size}>

<ForEach spacing=0.0 keys>
    {move |key: Uuid| view! { <ItemRow @sizing={row_size(key)} key /> }}
</ForEach>
```

- On the single root of a `view!`, `@sizing` makes it build a `ListChild` rather than a `NodeId`,
  which is what a `Dynamic`, `ForEach` or `Keyed` row builder returns, so each row picks its own
  size and may change it reactively.
- An `{expr}` child takes `@sizing` after the braces, so a node handed in (a render prop's result,
  a caller's `NodeId`) takes a share of a list without a wrapping `Frame`:

```rust
<List direction=Direction::Horizontal spacing=SPACING>
    <Spacer @sizing={indent} />
    {content.call(key)} @sizing=ItemSize::Percent(100.0)
</List>
```

- A percent child shares what is left over, so it needs a bounded main axis. In a list measured
  intrinsically there is no leftover, and percent children fall back to intrinsic length, like CSS
  `height: 50%` in an auto-height parent.
- A `scroll` measures as nothing, so a percent scroll in an intrinsically measured column
  collapses; give it a fixed length there.

`crates/beui-demo` has a full example. The behaviour relied on is tested in
`crates/beui/src/tests/a_reactive_tree_can_nest_builder_calls_without_threading_the_document.rs`
and `crates/beui/src/tests/a_signal_write_from_a_click_handler_updates_its_bound_text_in_the_same_frame.rs`.

## Controlled state

Anything a component would otherwise poke into a node after the fact is a prop on the base
element, so the component owns a signal and the node follows it.

| Element | Props | Write-back callback |
|---|---|---|
| `overlay` | `open`, `anchor` | `on_dismiss` |
| focusable `interactive` | `focused` | `on_focus_change` |
| `text_input` | `value` | |
| `scroll` | `offset`, `reveal` (index of the child to bring into view) | |

`unstyled::button`, `unstyled::toggle` and `unstyled::text_input` forward `focused` to their
`interactive`.

These props are edge triggered: the effect behind them runs when the value changes. To move focus
or close a popup, write the signal. Because the document also changes this state itself (a click
moves focus, a scrim click dismisses an overlay), pair the prop with its callback and write the
signal back; otherwise the signal goes stale and the next write of the value it already holds
changes nothing. `on_dismiss` reports only the document closing the overlay, never the owner
writing `open` false, so it can mean "cancel".

A `Selector` is the natural source for `focused` in a list: one signal names the row that should
have focus, and each row gets `focused={selection.memo(Row(index))}`, so moving focus wakes two
rows.

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

## Context

`provide_context(value)` stores a value on the current scope, keyed by its type.
`use_context::<T>()` walks up the owner chain and returns the nearest one, or `None`.

- Component scopes form the same tree as the nodes, so a component reads what its ancestors
  provided. Effects it creates later (a `show` branch built long after the first frame) see the
  same values, since a computation's execution scope is parented to the scope that created it.
- A scope holds on to the scope it was opened in, so a row a keyed list or `Show` keeps across its
  effect's reruns still reads what was provided above it.
- Provide a distinct wrapper type per concern, not a bare `f32`, or two providers collide.

Ordering: `view!` builds a tag's children before the tag, so a subtree passed as `Children` or
`Child` is already built before the provider's body runs. A provider therefore takes the subtree
as a render prop marked `#[prop(children)]` and calls it after `provide_context`. The caller still
writes the subtree between the provider's tags.

## Containers and responsive layout

Content that depends on the space it is given is built inside layout, not after it. A
`LayoutBuilder` hands its render prop a `ReadSignal<Vec2>` of the size it is laid out at and
rebuilds its content in the same walk when that changes (see "One layout per frame" in
`guides/beui/views.md`). `node_rect(id)` watches where a node was placed, for content positioned in
document coordinates; content that needs a rect to lay itself out uses a `LayoutBuilder`.

`unstyled::container` is a `LayoutBuilder` that builds its content in its own scope under the
caller's (like `Show` and `Dynamic`), so the caller's context still reaches it. It provides its
size as `ContainerSize` and hands the signal to its `content` render prop. Below it:

- `container_size()` reads that size.
- `narrower_than(width)` is a `Memo<bool>`, true when the nearest container is narrower than
  `width`; false when nothing provides a size, and until the first layout.
- Queries answer for the nearest container, so a card that wraps its contents in one gets answers
  about the card, not the window.

`unstyled::stack` is a row that becomes a column while that flag is true, its children falling
back to intrinsic sizing. It keeps the same nodes across the switch, so their state survives.
`styled::stack` defaults the breakpoint to `theme::NARROW_WIDTH`, and `styled::responsive_tabs`
swaps a tab bar for a select below one.

Measuring an intrinsically sized container whose children react to its width can oscillate. Give
containers a width that comes from their parent.

## How the graph meets the Document

### Nodes belong to scopes

Each `Document` owns a root `reactive::Scope` (`Document::reactive_scope`). Every node belongs to
the scope current when it was created, as an effect does, and disposing that scope removes the
nodes it still owns: a scope's nodes, effects and cleanups go together.

- Every `#[component]` runs in a `child_scope` of the scope that built it, so it lives exactly as
  long as its caller. Code built directly in `build`'s closure belongs to the root scope and lives
  as long as the document.
- `Show`, `Dynamic`, `Keyed`, `ForEach` and `VirtualList` build each child in its own scope and
  drop that scope to drop the child. Anything else that drops part of a view on its own keeps a
  `Scope` for it and drops that, as the dock does per tab panel. Nothing removes nodes by hand.
- A component that returns a value rather than a node lives as long as the scope that built it;
  dropping the value does nothing.
- The scope that owns a node and the node that holds it can differ. When a node is removed, its
  children owned by another live scope are detached, not removed, and live until their own scope
  goes; that is what lets a `Portal` show a subtree built elsewhere. Conversely, a node whose scope
  is disposed while a parent still holds it is taken out of that parent: every `Element` says how
  to let go of a child with `remove_child`.
- An effect that writes one particular node belongs with that node: `bind(node, || ...)` creates it
  in the scope that owns the node, so it stops when the node goes.
- A `NodeRef` lets go of its node when the node is removed, so `try_get` finds nothing rather than
  a removed id.

Disposing a scope never removes its nodes on the spot. Each node's cleanup queues its id with the
document that made it, and the document removes them once the disposing code has returned: after
each event, after layout, and when `with_reactive_scope` returns. So a node never leaves the arena
while something walks it, and a scope disposed while another document is installed still removes
its nodes from its own.

### The ambient document

No function holds `&mut Document` across a call boundary. Each reads it from a thread-local
(`with_document`) installed by the active ambient context and releases it before returning.

- `build` installs the document and enters its scope (so a `Prop` bound to a signal can create
  its effect) for the tree-building closure.
- `Document::show` installs itself for each event it dispatches and flushes the effects that event
  queued before the next one, outside any walk, so a signal write from a click handler is visible
  in the same frame and to later events.
- A write inside a batch is seen by effects when the outermost batch ends. The one exception is
  `ScopeContext::flush_owned` (and `Scope::flush_owned`), which runs at once the queued effects
  owned by that scope and those under it and leaves every other effect queued.
  `KeyedItems::entry_owned` builds an entry and flushes its scope that way. A node uses this to
  build its own subtree during layout; nothing else should need it.
- `with_document` asserts when no document is installed; nested calls are fine and reach the same
  document. Effects always run after the batch that queued them released its borrow, so ambient
  calls inside an effect nest safely.
- Event handlers are plain `FnMut` closures with no document parameter. Keep ambient calls at the
  leaves: a component reads and writes node properties through props on base elements, and uses
  `with_document` only inside a base element or to look up its own component state.

## Zones and pausing

### Zones

`enter_zone(zone)` enters a zone until its guard drops. An effect remembers the zone entered when
it was created, and effects it creates inherit it.

- While a different zone is entered, an effect woken by a write is parked and runs when its own
  zone is next entered. With no zone entered, every effect runs normally.
- `zone_pending(zone)` says whether a zone has parked effects; `forget_zone(zone)` drops them.

beui gives each `Document` its own zone and enters it whenever the document is installed. That
lets two documents share signals: a write made while one is installed does not rebuild the other's
nodes inside it, and the other catches up the next time it is shown. A host driving several
documents should give the others a frame when `zone_pending` says one is behind.

### Pausing

`Scope::detached().pausable()` (or `Scope::new().pausable()`, before anything is created in it) is
a scope that can be paused.

- While `pause()` holds, an effect under it that is woken waits; `resume()` runs each waiting one
  once, with the values as they are by then.
- Memos are not paused, but compute only when read, so a memo read only by paused effects waits
  with them.
- An effect already woken when the scope pauses still runs, so the write that hides a subtree
  reaches it: an overlay inside closes rather than staying open.
- A scope inside a paused scope waits too, whether or not it is pausable itself.

This keeps a hidden `ShowKeepAlive` child from doing work nobody can see.

## Block content in a plugin editor

A plugin editor reads its block's content through a `ContentProjection` (`block_editor_beui`).
The rest of the editor API is in `guides/plugins/editor_api.md`.

- `editor.block_content::<C>()` is the editor's own block; `editor.content_of::<C>(block)` is
  another block it follows. The plugin framework pumps both once at the top of every frame, inside
  the document's reactive scope; never pump by hand.
- A projection is a `Copy` handle the `Editor` owns and frees with itself. One made outside an
  editor (as `EditorHost::content_of` makes for an artifact's regeneration) is an
  `OwnedContentProjection`, which frees it on drop and derefs to the handle.

```rust
let checklist = editor.block_content::<ChecklistContent>();
let done = checklist.project(|checklist| checklist.root().done_count());
let ids = checklist.ids(ObjectId::ROOT, Checklist::ITEMS);
let item = checklist.object::<ChecklistItem>(id);
let count = counter.field(ObjectId::ROOT, Counter::COUNT);
```

A projection is a plain function of the content as the view sees it now; it writes its signal,
which notifies only when the computed value changed. Deriving from the current value rather than
from operations is what keeps it correct: when an edit from elsewhere arrives while one of this
editor's own is in flight, the visible content is rebuilt from the confirmed content plus pending
edits and can move in ways no single operation describes.

A projection runs only when something it watches was touched; applying an operation reports what
it touched.

| Method | Watches |
|---|---|
| `project` | Everything. |
| `project_on(key, ...)` | One key. |
| `project_keyed` | Fills a `KeyedStore`. |
| `field(object, FIELD)` | One field of one object (`be-model` documents). |
| `ids(owner, LIST)` | The order of one list. |
| `object::<T>(id)` | One object and everything inside it. |

The checklist's rows each watch their own item and the list only its order, so ticking one item
runs one row. A watcher is dropped with the reactive scope that made it.

Rules:

- Keep a projection cheap and pure; it must not operate.
- Write through `ContentProjection::operate`. It applies the operation to what the view sees at
  once (a local edit shows in the frame that made it) and queues it for the host. It is already
  gated on whether the host says the block may be edited, and dropped until the host has sent the
  content once, since an edit against content that has not arrived would land in the wrong place.
- `read` answers the content outside a projection (`None` until it has arrived); `loaded()` is a
  signal that turns true when it has.
- Until then show `block_editor_beui::Loading`, not an empty or editable view, and keep showing it
  until everything the editor draws from has arrived, so nothing is shown and then corrected (the
  database view waits for its database, schema, relations and row labels).
- `RelatedContent::settled()` and `content_settled` turn true once a block's content has arrived
  or the account cannot read it, so a block that will never arrive does not leave an editor
  loading forever.

The graph around a block (children, what it references, what references it) is a query, not
content: `editor.watch_blocks(BlockQuery::Children(id))` is a memo of the `BlockInfo`s the host
answers with, `None` until the first answer, changing whenever the answer does.

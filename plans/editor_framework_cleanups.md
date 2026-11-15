# Editor framework cleanups found in the text app

Depends on: none.

Reading `crates/be-app-text/src/root.rs` (`TextEditor`) turned up several places where an app works
around something the framework should do. Each part below stands alone and can land as its own
pull request, except that part 3's sweep and part 4's sweep touch the same lines and are best done
together (part 3, step 4). None of them changes `editor.content(&node_ref)`: that is deliberately
left as it is. Part 1 (replacing a child is the type's job, not the editor's) and part 2 (typed,
reactive view state) have landed; the parts keep their numbers.

## 3. `Show` with a fallback, for runs and for single children

**Today.** `Show` (`crates/beui-tree/src/control.rs:32`) takes `condition`, `keep_alive` and `then`,
and returns a `DynamicSegment<C>`: a run of children that only fills a place inside a parent that
keeps a run (`List`, `Layers`, ...). There is no fallback, so an either/or is written as two `Show`s
and a negated memo:

```rust
let text_shown = create_memo(move || !hex.get());
let hex_shown = create_memo(move || hex.get()); // redundant: a Signal is already a prop
view! {
    <List spacing=Space::None>
        <Show condition={text_shown} then={text_surface} />
        <Show condition={hex_shown} then={hex_surface} />
    </List>
}
```

(`be-app-text/src/root.rs`; `root/toolbar.rs` `EditorMenu` and `menu_actions` repeat it). Outside
tests there are about 870 `<Show` uses, 156 files with two or more, and about 275
`create_memo(move || !...)` lines.

A single-child slot cannot take a `Show` at all. Such slots take a built `NodeId` (`Child` in
`crates/beui-view/src/reactive.rs`; `Render<()>` children such as `be_sdk_beui::Loaded`'s;
`children: Option<Child>` on `Frame`, set once with `Document::set_frame_child` in
`crates/beui-view/src/native/frame.rs`). Passing a segment there fails at compile time with the
`FixedChild` diagnostic in `crates/beui-tree/src/children.rs`. That is why `TextEditor` wraps its
two `Show`s in an extra `List` and each branch in a `ListItem`, and why `Loaded` itself wraps
`Loading` in `Layers`.

**What must work**, with one `Show` and the same syntax:

```rust
// A run: each branch builds any number of children, and the count may differ.
<List>
    <Show condition={editing} fallback={|| view! { <ListItem /> <ListItem /> <ListItem /> }}>||
        <ListItem />
        <ListItem />
    </Show>
</List>

// A single-child slot: each branch builds exactly one child.
<AcceptsOneChild>
    <Show condition={editing} fallback={|| view! { <ItemOne /> }}>||
        <ItemTwo />
    </Show>
</AcceptsOneChild>
```

(The exact spelling of `fallback` and the children block may differ; follow how `view!` already
writes render props and children.) Whether a `Show` can fill a single-child slot comes from its
branches' types, not from `Show`: it is one child when *both* branches build exactly one, and zero
or one when there is no fallback and `then` builds one (accepted only where a slot is optional,
like `Frame`'s `Option<Child>`). Every mismatch (a branch that builds a run, or a missing fallback,
put in a slot that needs exactly one) must stay a compile-time error with a clear
`#[diagnostic::on_unimplemented]` message, as `FixedChild`, `OneChild` and `AtMostOneChild` give
today. No wrapper node may be added to make it work: it would cost a node per use and change
layout.

**Steps.**

1. **Single-child slots that can change.** A single-child parent must be able to swap its one
   child in place. Add the single-slot counterpart of `SlotHost`/`ChildSlot` in beui-tree (a host
   whose only operation is "this is now the child", or "there is none"), let `Child` (or the type
   single-child props take) be either a built node or something that installs into such a host,
   and implement the host in both cores for every node kind that keeps one child (native
   `beui-core`/`beui-view/src/native`, and `beui-core-html`/`beui-view/src/html`). Check that the
   accessibility tree and damage follow a swapped child, as they do for a refilled run slot.
2. **`fallback` and the branch types.** Add `fallback` to `Show` (built when the condition is false,
   dropped when it turns true; with `keep_alive` both branches are kept, each paused while hidden).
   Make `Show`'s branch type generic over what it builds so that one component yields a run segment
   for run branches and a single swappable child for one-child branches. This likely needs
   `#[component]` and `view!` (`crates/beui-macros`) to let a component's return type follow its
   props' types; change the macros as needed. Do the same for `Dynamic` and `Keyed`, whose one
   render always builds one child when it builds one node.
   If a single `Show` cannot be made to serve both while keeping every error at compile time, fall
   back to two names (for example `Show` for runs and a single-child form), which is acceptable
   but not preferred; record the reason as a decision in decisions/.
3. **Let `Loaded` and other `Render<()>` children take it.** `Loaded` should accept a one-child
   `Show` (or `Dynamic`) directly; and drop its own `Layers` wrapper if step 1 allows `Loading`
   to fill a single slot.
4. **Sweep**, together with part 4's: replace each pair of `Show`s on a condition and its negation
   with one `Show` and a `fallback`, drop the negated memo it no longer needs, and remove
   `List`/`ListItem` wrappers that only existed to host a segment. Start with `be-app-text`
   (`root.rs`, `root/toolbar.rs`). Update the paintings this changes (`./scripts/verify` accepts
   them; look at the diffs, which should be none where only the code's shape changed).
5. Add a `fallback` example to beui-demo, tests in beui-tree for both slot kinds (built, swapped,
   dropped, kept alive), and a compile-fail test for each mismatch in `compile_fail/beui/`
   (guides/buck2.md, compile-fail tests). `compile_fail/beui/a_show_is_not_a_frame_child.rs`
   expects a one-child `Show` inside a `Frame` to fail; that becomes valid (a `Frame` takes zero or
   one child), so rewrite it to put a `Show` whose branch builds a run there, and add one for a
   `Show` without a fallback in a slot that needs exactly one. Update guides/beui/core.md and
   guides/reactive.md where they describe `Show`.

**Done when** both examples above compile and behave, each wrong use fails to compile with a
message that says what to do, and no either/or in the repository is written as two `Show`s.

## 4. A closure is a prop

**Today.** A prop is a `Prop<T>`, filled through `IntoProp<T>` (`crates/beui-tree/src/prop.rs`),
implemented for a plain `T`, `Prop<T>`, `Signal`, `ReadSignal` and `Memo`. There is no impl for a
closure, because the blanket `impl<T> IntoProp<T> for T` would overlap with one for `F: Fn() -> T`.
So `condition={move || !hex.get()}` does not compile, and code makes a memo only to pass a
derived value (`Prop::dynamic(..)` works but is rarely used).

**Steps.**

1. Make `prop={move || expr}` compile for every `Prop<T>` prop, and keep closures passed to
   callback props (`on_click={move || ..}`, `IntoUnitCallback`) working as they do. Candidate
   ways, to be tried in this order: autoref specialization in the code `view!` generates for a
   prop value (the value's type is concrete at the call site); replacing the blanket `for T` impl
   with a marker trait value types implement; or having `view!` wrap a literal closure written as
   a prop value in a marker type with its own `IntoProp`. Whichever is used, a wrong type must
   still be a compile-time error with a readable message; add a case to `compile_fail/beui/` for
   a closure returning the wrong type.
   If none works without making errors worse, stop, record why in decisions/, and do only the
   `Prop::dynamic` part of the sweep below.
2. **Sweep** (with part 3's): replace memos that exist only to pass a derived value to a prop with
   a closure. Keep a memo where it stops repeated signals: when its inputs change more often than
   its output (a boolean derived from a scroll offset or a long list, say) or when computing it is
   expensive and it is read in several places. Don't replace a memo passed straight through as a
   `Signal`/`Memo` (`condition={hex}` needs neither).
3. Say in guides/beui/core.md how to pass a derived value and when a memo is still the right
   choice.

**Done when** a closure works as a prop value, and memos that only fed one prop are gone.

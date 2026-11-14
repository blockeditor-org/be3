# Editor framework cleanups found in the text app

Depends on: none.

Reading `crates/be-app-text/src/root.rs` (`TextEditor`) turned up five places where an app works
around something the framework should do. Each part below stands alone and can land as its own
pull request, except that part 3's sweep and part 4's sweep touch the same lines and are best done
together (part 3, step 4). None of them changes `editor.content(&node_ref)`: that is deliberately
left as it is.

## 1. Replacing a child is the type's job, not the editor's

**Today.** The block menu's "Unlink" (`BlockAction::Unlink` in `crates/be-sdk-shell/src/menu.rs`,
`BlockCommand::Unlink` in be-sdk-api) is offered on a *reference*: a block shown inside a container
whose parent is elsewhere. It gives the container its own copy: the host duplicates the block
(`HostCore::queue_copy` and the `CopyStage` loop in `crates/be-host/src/main.rs`), then repoints the
container's content from the original's id to the copy's (`AppEditor::replace_child`,
`crates/be-host/src/editors/app.rs:450`). That repointing first asks the open editor over the app
protocol (`replace-child` in `crates/be-sdk-api/wit/app.wit:94`, answered by
`editor.on_replace_child`), and only if the editor says no falls back to the type module
(`ChildChange::Replace`, `crates/be-sdk-type-api/wit/types.wit:206`, reaching
`Root::child_edit`).

The text editor answers it (`root.rs`, `State::replace_child` in `root/state.rs:248`, rewriting
the UUID inside block URLs found by `reference_ranges` in `crates/be-app-text/src/document.rs`).
The infinite canvas answers it too (`replace_referenced_block` in
`crates/be-app-infinite-canvas/src/root/state.rs`), duplicating what its type already does in
`Canvas::child_edit` (`crates/be-type-infinite-canvas/src/canvas.rs:1378`). `TextBlock`
(`crates/be-type-text/src/text.rs`) has no `child_edit`, so text only works while its editor is
open. An editor should not need to know about this at all: the edit reaches an open editor through
its replica like any other change.

**Steps.**

1. Give `Root::child_edit` (`crates/be-block-content/src/model.rs`) the client id it runs as, since
   a text sequence edit needs one (`TextBlock::replace(content, client, range, bytes)`). The type
   guest already has it: `apply(kind, bytes, client, change)` in
   `crates/be-sdk-type-guest/src/lib.rs` drops it before calling `child`. Thread it through
   `LiveEdit::child_operations` and `child`, and update every `child_edit` implementation
   (workspace folders, chat, canvas, `EditorView`).
2. Implement `child_edit` for `TextBlock`: `Replace { old, new }` rewrites the UUID in every block
   URL naming `old` (move or share the range finding from `be-app-text/src/document.rs` into
   be-type-text or `be_block_content::block_url`, beside `parse_block_urls`). `Delete` can stay
   `None` unless deleting should blank the URL; keep whatever text does now.
3. Delete the editor path: `replace-child` in `app.wit`, `EditorCall::ReplaceChild` and its wire and
   runtime handling (`crates/be-host-runtime/src/{calls,wire,runtime,instances,lib}.rs`),
   `App::replace_child` and its plumbing in be-sdk (`app.rs`, `wire.rs`, `editor_session.rs`,
   `editor_state.rs`), `Editor::on_replace_child`/`replace_child` in
   `crates/be-sdk-beui/src/editor.rs` and `instance.rs`, `replace_child` in
   `crates/be-sdk-test/src/beui.rs`, and both apps' handlers. `AppEditor::replace_child` in
   be-host becomes `change_child(Replace)` gated on the manifest's `children.replace`, as
   `reorder_child` is.
4. Tests: move `be-app-text/src/root/tests/replacing_a_referenced_block_rewrites_its_url.rs` and
   the canvas's `replacing_a_referenced_block_rewrites_the_entity.rs` to the types' crates (be-type-text
   already has tests; the canvas type has `canvas_children_are_removed_repointed_or_merged.rs`,
   which may already cover it). `a_rewrite_refused_while_read_only_leaves_the_text_as_it_was.rs`
   uses `replace_child` only as a way to cause a rewrite; drive the rewrite another way, or delete
   it if nothing else rewrites the text while read-only.
5. Remove the `on_replace_child` paragraph from `guides/apps/editor.md` (Presence), which also
   describes it wrongly, and say in guides/type_modules.md or guides/blocks/content_types.md that
   a type whose references live in its content implements `child_edit` `Replace`.
6. Rename the menu item: "Unlink" reads as "remove the link", but it makes an independent copy in
   place. Label it "Make a copy here" and rename `BlockAction::Unlink`, `BlockCommand::Unlink`,
   `unlink_block` and the `unlink` permission to match (no saved data uses these names).

**Done when** unlinking a reference inside a text block works with the text editor closed and open,
no app registers anything for it, and `replace-child` is gone from the app protocol.

## 2. Typed, reactive view state, saved on every change

**Today.** View state is a `LatestMap<String, ViewState>` in the view block
(`be_block_content::EditorView`, `crates/be-block-content/src/editor_view.rs`); `ViewState` is
opaque postcard `bytes` plus `refs`, block ids the type repoints or blanks when a child is replaced
or deleted (`EditorView::child_edit`), so bytes may index into `refs`. Editors read it with
`editor.view_state(key)` (a one-off read) or by reading `editor.view_content()` inside a binding,
and write with `editor.set_view_state(key, Some(&ViewState::new(&value, refs)))`. Keys are string
constants; values are decoded by hand with `ViewState::value::<T>()`.

`crates/be-app-text/src/root/position.rs` (`keep_position`) shows what that costs: a
`StoredCell` "restored" flag and an early-returning binding to wait for the view block, a
`RefCell<Option<Position>>` to skip unchanged writes, a 500 ms timer (`SAVE_AFTER`) to debounce
saves, and an extra effect to save when the editor is hidden. The file tree (`root/zoom.rs`,
`root/rows.rs`) has the same restored-flag pattern. Callers today: be-app-text, be-app-file-tree,
be-app-workspace (`FILES_VIEW`), be-app-boat (`CACHE`), be-app-desktop (`DISPLAY_BARS`,
`CALENDAR_STATE`), be-sdk-shell (`LAYOUT`, `TITLES`, `RECENTS`, `CLOSED_VIEWS`, in
`workspace.rs` and `saved.rs`).

**Design.**

- **Typed keys.** A view-state value is a type implementing a trait in be-block-content beside
  `PresenceKind` and `ChannelKind` (`src/presence.rs`), which identify themselves with
  `const ID: Uuid`:

  ```rust
  pub trait ViewStateKind: Serialize + DeserializeOwned + PartialEq + 'static {
      const ID: Uuid;
  }
  ```

  The map is keyed by that id (`LatestMap<Uuid, ViewState>`; change the stored form freely, no
  format is frozen). Each kind stays its own map entry, so two devices writing different kinds
  (the shell's layout and its recents) still merge without overwriting each other. Do not fold an
  editor's view state into one struct.
- **Block references stay typed.** Values name blocks through a `ViewRef` (a newtype over `Uuid`)
  that serializes as an index into the entry's `refs`, so the type module can still repoint or
  blank them without knowing the value's type. Writing collects the refs while serializing;
  reading resolves each index (a blanked ref reads as `None`, so a field is `Option<ViewRef>` or
  a list filters them). A thread-local or a serializer wrapper during the postcard call is enough.
  `saved::SavedLayout`, `ClosedView`, `FILES_VIEW` and `CALENDAR_STATE` then hold `ViewRef`s
  instead of hand-kept indices and parallel `refs` vectors.
- **One reactive handle.** `editor.view_state::<K>()` in be-sdk-beui returns a `Copy` handle:
  - `get()`, tracked, returns `Ready<Option<K>>`: `Err(Pending::Loading)` until the view block is
    read, `Ok(None)` when nothing is saved. A value that is saved but does not decode is not
    `None`: the shell keeps an unreadable layout rather than replacing it
    (guides/blocks/workspaces.md, `unreadable_layout()`), so a decode failure is
    `Err(Pending::Failed(..))` or a distinct variant the shell can tell apart.
  - `set(Option<K>)` writes at once, skipping a value equal to the one held (as
    `EditorState::set_view_state` already does in `crates/be-sdk/src/editor_state.rs`).

  `editor.view_state(&str)`, `set_view_state(&str, ..)` and `ViewState::new`/`value` stop being
  public API; nothing reads a view state any other way.
- **No debouncing.** Saving on every scroll event or keystroke is expected and must be cheap. If a
  stream of writes costs anything (outbox size, history, sync traffic), fix it where it is stored,
  never in callers: `Document::absorb_operation` (`crates/be-block-content/src/model.rs`) merges
  consecutive text edits but not consecutive `LatestMap` puts to the same key; make a put absorb
  an earlier unsent put to the same key so a burst becomes one operation. Measure a burst of writes
  in a test that counts the outbox's operations.

**Steps.**

1. Add `ViewStateKind`, `ViewRef` and the uuid-keyed map, with tests in be-block-content for
   writing, reading, a ref repointed and blanked by `child_edit`, and an undecodable value.
2. Add the put absorption and its test.
3. Add `editor.view_state::<K>()` in be-sdk-beui (and whatever be-sdk-test needs to read it in
   tests), and remove the string-keyed API.
4. Port every caller listed above. For text, `keep_position` becomes: a memo of `Position::of`
   keyed on `scrolled` and `cursors` (the surrounding text stays read untracked, which is what
   finds a place again after edits), an effect that `set`s it whenever it changes, and a binding
   that restores once when `get()` first returns `Ok`. `state.position_restored` stays: an opened
   search hit waits for it (`select_where_opened` in `root/surface.rs`). Delete `SAVE_AFTER`, the
   timer, the `saved` cell and the hidden-editor effect. Keep
   `a_reopened_text_opens_scrolled_where_it_was_and_keeps_its_caret.rs` passing.
5. Update guides/blocks/workspaces.md (View state) and guides/apps/editor.md, including the rule:
   view state is written on every change and never debounced.

**Done when** no caller names a view state with a string, decodes bytes or indexes `refs` by hand,
and nothing in the repository delays a view-state write.

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

## 5. A failed action is reported by the host, not drawn by the app

**Today.** Apps keep an error signal and draw their own dismissable `Banner` for one-off
failures:

- be-app-text: `ImportError` (`root/import_error.rs`) shows `state.import_error`, which nothing
  ever sets to `Some` (the only write is `.set(None)` in `root/surface.rs`). It is dead code.
- be-app-infinite-canvas: `state.import_error` (`root/state.rs`), set when copying canvas objects
  cannot be serialized, when a pasted clip names the wrong number of blocks, and when a picked image
  file cannot be read; drawn in `root/toolbar.rs`.
- be-app-database: `data.error` (`database_view/root/data.rs`), set when picking a block for a
  cell fails; drawn in `database_view/root/header.rs`.

(be-app-deterministic-game's `Banner` is its own component for the game's table, not this.)

The host already shows one-off failures as toasts: `be_host_base::notices::report(message)`,
shown by `crates/be-host/src/ui.rs` (`notice_toasts`). Apps have no way to reach it.

**Steps.**

1. Add an app-protocol call for it, for example `report-failure: func(message: string)` in
   `crates/be-sdk-api/wit/app.wit`, handled in be-host-runtime by `notices::report`, exposed as
   `editor.host().report_failure(message)` in be-sdk and be-sdk-beui, recorded by be-sdk-test so
   tests can assert on it. Keep the protocol free of beui types. Document it in
   guides/apps/host_services.md.
2. Delete `ImportError`, `import_error` and its `.set(None)` from be-app-text.
3. Port the canvas and the database view to the new call and delete their error signals, dismiss
   methods and banners. Update their tests and paintings.

**Done when** no app keeps an error signal or a `Banner` for a one-off failure.

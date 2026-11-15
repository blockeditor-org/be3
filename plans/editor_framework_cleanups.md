# Editor framework cleanups found in the text app

Depends on: none.

Reading `crates/be-app-text/src/root.rs` (`TextEditor`) turned up places where an app works
around something the framework should do. Each part below stands alone and can land as its own
pull request. None of them changes `editor.content(&node_ref)`: that is deliberately left as it
is. (Parts 3 and 4, `Show` with a fallback and closures as props, have landed.)

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
   Keep the name "Unlink" for the menu item and the action.

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

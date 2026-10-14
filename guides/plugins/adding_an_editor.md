# Adding a plugin editor

How to add an editor for a block type, step by step, and the reference for its `manifest.json`.
Read this when you create a new editor plugin. What an editor can do once it exists is in
`guides/plugins/editor_api.md`; how plugins work underneath is in `guides/plugins/architecture.md`.

## Editors are plugins

Every editor in the app is a plugin, and so is the app's own window:

- block-app itself only signs the user in, switches workspaces, hosts plugins, owns the app menu
  (a sheet that opens when a plugin calls `EditorHost::show_app_menu`, as the account button in
  the files toolbar, or the files page's header on a phone, does) and owns the dialogs a plugin
  cannot draw for itself.
- Everything else on screen (the dock of tabs, the pane of files, the block's status bar) is
  `crates/editors/workspace-ui`, an ordinary plugin. The app opens it on the client's session, a
  view block under the account's settings block (`guides/blocks/in_the_app.md`, settings and
  sessions), and it draws filling the window. It places the file tree and the block each tab
  shows as children of its own, each with a frame of its own, so every editor on screen is a
  plugin instance the app only composites.
- Narrower than `NARROW_WIDTH` the workspace stacks its dock (`DockMode::Stacked`) with Files as
  the home tab: one screen at a time, the file tree or one open block, under the dock's stacked
  bar (back to Files, the switcher, and the More button of the block on show). The same tabs,
  and the editors in them, stay open when the window crosses `NARROW_WIDTH`.
- The only editor left in block-app is the placeholder for a block type no plugin claims.

## Examples to copy from

| Editor | Shows |
|---|---|
| `crates/editors/counter` | the smallest beui editor |
| `crates/editors/checklist` | a larger retained document; the `Cargo.toml` and `BUCK` pattern |
| `crates/editors/deterministic-game` | dialog creation with a host block picker |
| `crates/editors/presentation` | children, playback and a preview |
| `crates/editors/pan-zoom` | pan and zoom; its block stores nothing |
| `crates/editors/triangle` | a plugin with no ui framework at all |
| `crates/editors/logic-grid` | one plugin editing several block types |
| `crates/editors/block-debug` | an editor for every block type (`"any"`) |
| the database, worktree, game module and audio editors | chrome that works on desktop and phone |
| the pixel art export, compiled logic (logic-grid) | dynamic artifacts |
| `crates/editors/scene-3d` | pointer lock |
| `crates/editors/browser-tab` | web views |
| `crates/editors/paint-review` | `fetch` |
| `crates/editors/pdf` | work on a thread, a C library linked whole |

## Steps

### 1. Add the block, if the block type is new

The content type lives in `crates/be-block` and is registered in `KINDS` in
`crates/block-app/src/be.rs`; `guides/blocks/adding_a_block.md` covers it end to end. That
registration is the one place outside the plugin that learns about a new type. A plugin cannot
bring a block type of its own, which is what keeps child edits, undo and duplication working.

A block holds what it is, not what an editor makes of it. The image block keeps the file's bytes
and a header saying what decoding them found (or why it failed), which the editor fills in
through an operation the first time it draws the image. Then nothing but the plugin needs a
decoder, and no other client has to decode the block to know its shape.

A block type with no state at all can be one line of be-block's `kind!` macro
(`be_block::TriangleContent`).

### 2. Create the package

1. `crates/editors/foo/Cargo.toml`: copy checklist's.
   - `crate-type = ["cdylib", "rlib"]`.
   - Dependencies: `block-editor-beui`, `beui` (`default-features = false, features = ["render"]`)
     and `uuid`; `block-ui-test` as a dev-dependency.
   - `block_editor_beui` re-exports `be_block`, so the editor names its content type without
     depending on be-block itself.
   - The package name is the wasm artifact stem, so a multi-word editor uses underscores
     (`workspace_index`). It must not clash with any crate the workspace already depends on, so
     the image editor's package is `image_block`. The display name in the manifest is what the
     user sees.
2. Root `Cargo.toml`: add `"crates/editors/foo"` to `members` (see `guides/buck2.md` for what
   else adding a crate needs).
3. `crates/editors/foo/src/lib.rs`:

   ```rust
   pub mod app;
   block_editor_beui::beui_plugin!(app::FooApp, "../manifest.json");
   ```

4. `crates/editors/foo/src/app.rs`: implement `block_editor_beui::BeuiApp` (below).
5. `crates/editors/foo/manifest.json`: see [The manifest](#the-manifest).
6. `crates/editors/foo/BUCK`: one `editor()` call naming the module the manifest's
   `entry_point` names. Nothing in block-app is touched to add a plugin.

   ```python
   load("@root//buck/wasm:defs.bzl", "editor")

   editor(
       module = "foo",
   )
   ```

### 3. Write the app

An editor has no state struct and no `connect`. It is one `#[component]` function handed an
`Editor`; the type named in `beui_plugin!` only carries that function and the few answers the host
asks of the editor as a whole:

```rust
pub struct FooApp;

impl block_editor_beui::BeuiApp for FooApp {
    fn view(editor: Editor) -> NodeId {
        view! { <Foo editor={editor} /> }
    }

    fn create_block(creation: &Creation) -> Result<Uuid, String> {
        Ok(creation.create(&FooContent::default()))
    }
}
```

| `BeuiApp` item | Needed when |
|---|---|
| `view(editor)` | always: the editor's frame |
| `create_block(creation)` | the editor's templates make blocks (`guides/plugins/editor_api.md`, Creating blocks) |
| `creation_view(creation)` | a template has `"dialog": true` |
| `preview_view(editor)` | the manifest lists the `Preview` region |
| `connect_artifact`, `describe_artifact`, `artifact_settings_view` | the manifest lists `ArtifactSettings` |
| `intrinsic_size()`, `aspect_ratio()` | the editor has a preferred size or shape when embedded |

Inside `view`, the editor reads its block with `editor.block_content::<FooContent>()`, keeps local
state in signals, and writes back through the projection's `operate`.
`guides/plugins/editor_api.md` is the full surface, by topic.

### 4. Write the tests

Test the editor with `block_ui_test::BeuiTest`, built from the `Editor` (or from a `Creation` for
a dialog). It runs the editor through the same plugin session the shipped plugin runs, against a
stand-in host. `guides/testing.md` covers writing and running editor tests and their snapshots.

### 5. Build and verify

- `./scripts/verify` lints every editor as the wasm guest it ships as and runs its tests there.
- `./scripts/buck build //crates/block-app:web` builds the browser's half of the bootstrap and
  every plugin as the guest it ships as.
- `//crates/block-app:plugins` builds the plugins alone; `//crates/block-app:native` builds and
  stages them with the app (`guides/plugins/architecture.md`, Building and staging).
- Drive a plugin without a GUI: this takes it through the handshake, opens an instance, asks for
  a frame and fails unless it presented a surface.

  ```sh
  ./scripts/buck run //crates/block-wasm-host:example-instantiate -- \
    "$(./scripts/buck build //crates/editors/foo:module --show-full-output | tail -1 | cut -d' ' -f2)"
  ```

## Variations

### Several block types in one plugin

A plugin may edit several related block types (`crates/editors/logic-grid` edits logic grids,
logic games, hotbars and compiled logic). Each is one `BeuiApp`, and `lib.rs` names them by
content type:

```rust
beui_plugin!("../manifest.json", {
    LogicGridContent => app::LogicGridApp,
    HotbarContent => hotbar::app::HotbarApp,
    // ...
});
```

The framework opens each instance with the app of the block type the host names, and refuses to
start unless the apps cover exactly the editors the manifest declares. Related editors belong in
one plugin rather than one each: they share a module, and the host runs one runtime for them.

### A plugin with no ui framework

An editor that wants no beui implements `block_editor_plugin::Plugin` and `Instance` itself and
names its type in `block_editor_plugin::plugin!(Type, "../manifest.json")`.
`crates/editors/triangle` is the example: it draws one triangle with plain wgpu into the middle of
its surface, asking to be painted only until it has been. Its tests drive the `Instance` directly
and assert on what it asked of the `EditorHost`. The `Plugin`/`Instance` contract is in
`guides/plugins/architecture.md`.

### An editor that needs something the host does not offer

Extend the protocol: `guides/plugins/architecture.md`, Extending the protocol.

## The manifest

`manifest.json` is the plugin's single source of truth: the plugin reads its own identity out of
it and the host reads everything else.

```json
{
  "id": "be3.foo",
  "name": "Foo",
  "version": "0.1.0",
  "editors": [
    {
      "block_type": "00000000-0000-0000-0000-000000000000",
      "display_name": "Foo",
      "icon": "",
      "templates": {
        "main": {},
        "starter": {"name": "Starter foo", "icon": "", "category": "template"}
      },
      "regions": ["Frame"]
    }
  ],
  "entry_point": "foo.wasm",
  "network": []
}
```

A manifest is untrusted input: a bad one is skipped and reported in the Plugins debug window
rather than crashing the app.

### Top-level fields

| Field | Meaning |
|---|---|
| `id` | the plugin's id; an editor's id is `block_plugin_api::editor_id` of it |
| `name`, `version` | the plugin's name and version |
| `editors` | every block type the plugin edits, one entry each (below) |
| `entry_point` | the wasm module beside the manifest, where the build already put it. One artifact, the same bytes on desktop, Android and in the browser |
| `network` | host names the plugin may reach through `fetch`, matched whole against the host of an https URL; empty for a plugin that reaches nothing |

### Editor fields

| Field | Meaning | Default |
|---|---|---|
| `block_type` | the `CONTENT_TYPE` of a be-block `Root` registered in `KINDS` (the counter's is `Counter::CONTENT_TYPE`), or `"any"` | required |
| `display_name` | what the user sees | required |
| `icon` | a codepoint of the app's Material font, not a name to look up | required |
| `templates` | how the add-block menu offers this editor's blocks (below) | none: never offered |
| `regions` | which screens the host may give an instance (below) | |
| `accepts` | requests a shell editor takes from editors inside it (below) | none |
| `children` | which of the host's structural edits the block type accepts | plainest answer |
| `interaction` | whether an embedded instance is live, or only previewed until focused | plainest answer |
| `capabilities` | `rotation`, aspect ratio, `pan_and_zoom`, `max_zoom` | plainest answer |
| `resize` | how an embedded instance may be resized | plainest answer |

`regions`:

- `Frame`: every editor has it; the whole rectangle it is edited in.
- `Preview`: the editor draws its block wherever the host paints it (`preview_view`).
- `ArtifactSettings`: the plugin answers for the dynamic artifacts its blocks make.

`accepts` lists `show_block`, `pick`, `dialog`, `panel` and `window`. The host hands a request an
editor raises to the nearest editor around it, itself included, that accepts it, and the host's
own requests to the outermost shell, which must accept them all. An ordinary editor accepts none.

`capabilities.pan_and_zoom` gives the content a camera owned by the host
(`guides/plugins/pan_and_zoom.md`). `max_zoom` is how far past its fitted size the host lets a
pan-and-zoom editor be zoomed: 32 unless it says otherwise (the map asks for far more, to reach
street level).

### Templates

`templates` maps an id (unique within that editor) to how the add-block menu offers it, in the
order written. An editor with no templates is never offered: only another block ever produces
one.

| Template field | Meaning | Default |
|---|---|---|
| `name`, `icon` | the menu entry | the editor's |
| `category` | `important`, `regular` or `debug`: the menu's main tab, under the headings Common, More blocks and Debug. `template`: its Templates tab, grouped under the declaring editor's icon and display name | `regular` |
| `block_type` | the type the template makes, which may differ from the editor's (the presentation editor's slide templates make canvases); lets the host offer it to a picker that only takes that type | the editor's |
| `dialog` | the block cannot exist until the user has chosen something; the editor writes `creation_view` | `false` |

The plain block's template id is `"main"` (`Creation::MAIN`).

### Several editors for one block type

- The first plugin found that names a block type is its built-in editor: the type's display
  name, icon and children come from it, and it opens the type anywhere nothing says otherwise.
  Other editors are offered beside it.
- `block_type: "any"` declares an editor for every block type, with no templates of its own
  (`crates/editors/block-debug`, the Debug view; see `raw_block_content` in
  `guides/plugins/editor_api.md`).
- The host lists every editor with the block types it edits in its catalog (`Catalog::editors`;
  `BlockCatalog::editors_for`, `built_in_editor` and `editor` on the plugin side).
- `ChildTarget::edited_with(editor)` opens a child in that editor rather than the built-in one,
  replacing whatever editor the block was open in; a child that names none keeps whichever is
  open. Only the workspace's tabs name an editor so far; every embedded child opens in the
  built-in one.
- `open_block_with(id, block_type, editor)` is `open_block` for a chosen editor. It reaches the
  workspace as `ShowRequest::editor`: the file tree's Open with sends it, and the workspace
  switches the block's tab to that editor, with a view block of its own, and remembers the
  choice with the tab.
- A tab with no choice opens in the default the person set in the Default editors settings
  (`be_block::EditorSettings`, edited by the ui-settings plugin), or else the built-in editor.

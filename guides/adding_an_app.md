# Adding an app

Every editor in be-host is an app: a wasm module that runs beside the host (a
thread on V8, a worker in the browser), speaks the app protocol, and
hands back the surfaces it painted (guides/blocks/apps.md covers blocks on the
app protocol). The host signs the user in, hosts apps,
draws the host menu and owns the dialogs an app cannot draw for itself.
Everything else on screen, the dock of tabs and the pane of files included, is
`crates/be-app-workspace`, an ordinary app the host opens on the session's view
block (guides/blocks/workspaces.md, Sessions); it places the file tree and each tab's block as children, so every
editor on screen is an app instance the host only composites.

This guide is the how-to for writing one. The rest is under `guides/apps/`:

- [apps/editor.md](apps/editor.md): what an editor does inside its frame: the
  frame menu, chrome and side panels, phones, pan and zoom, previews, making
  blocks and artifacts, presence and accessibility.
- [apps/host_services.md](apps/host_services.md): what an editor asks of the
  host: the `EditorHost` calls, the graph, picking and renaming a block,
  watches and host calls, and adding to the protocol.
- [apps/children.md](apps/children.md): placing other editors inside yours:
  `Subregion` and `ChildBlock`, modes, focus, a child that takes the frame,
  docks that join across apps, program windows.
- [apps/framework.md](apps/framework.md): be-sdk and be-sdk-beui: views,
  windows, anchored views, input, fonts and the GPU, for work on the framework
  rather than one app.
- [apps/runtime.md](apps/runtime.md): how the host discovers and runs apps,
  frames and pacing, and what happens when an app stops.

Related: [app_libraries.md](app_libraries.md) (linking and the raw ABI),
[type_modules.md](type_modules.md) (the module that holds a block type's
replicas), [adding_a_block.md](adding_a_block.md) (the block type),
[beui/programs.md](beui/programs.md#beui-in-an-app) (beui inside an app).

## 1. The block type

A new block type lives in `crates/be-type-foo`, the app's types crate, built as
a library and as the app's type module. [adding_a_block.md](adding_a_block.md)
says how; `crates/be-type-checklist` is the example. Nothing outside the app
learns about the type except through the manifest.

## 2. The package

- `crates/be-app-foo/Cargo.toml`: copy checklist's (`crate-type = ["cdylib",
  "rlib"]`, be-sdk-beui, beui, the types crate, be-sdk-test as a
  dev-dependency). `be_sdk_beui` re-exports `be_block` and the core, so the
  editor depends on nothing else of the framework.
- `src/lib.rs`: `be_sdk_beui::beui_app!("../manifest.json", { foo => root::FooRoot });`
  pairs each editor the manifest names, and each host component (its `hosts`,
  guides/apps/host_services.md, "Host components"), with the `BeuiRoot` that
  runs it. The
  framework refuses to start unless the roots cover exactly the manifest's
  editors.
- `src/root.rs`: `impl be_sdk_beui::BeuiRoot`.
- `manifest.json`: section 4.
- `BUCK`: `cargo_package()` and one `app()` call naming the module the manifest's `entry_point`
  names and `types = "//crates/be-type-foo:module"` (copy
  `crates/be-app-checklist/BUCK`).
- Root `Cargo.toml`: add both crates to `members`, then
  `./scripts/buckify`.

Every crate named `be-app-*` is staged as an app; nothing in be-host changes.
One app may hold several editors (`crates/be-app-logic-grid` has a grid, a
hotbar and more), each its own `BeuiRoot`, sharing one module and one runtime;
two may edit the same type.

## 3. Write the editor

An editor has no state struct and no `connect`: it is one component,
`BeuiRoot::View`, handed an `Editor`, a `Copy` handle that closures take with
`move`. The instance owns the state behind it and frees it after the views
built on it. The `BeuiRoot` type also answers `intrinsic_size` (or
`editor.set_intrinsic_size` when it changes); leave it out for the host's default.

`View` returns a `<Root>` (its `clear_color` is what the frame shows where the
editor paints nothing) around `<Loaded label="Loading the foo">`, which keeps
the view hidden until what it reads has arrived. Read the content inside
`<Loaded>`, not in `View` around it (guides/reactive.md, Loading).

### Content and state

`editor.block_content::<FooContent>()` is a `ContentProjection` of the
block's replica in its type module. Bind what you read as projections of it
and write back with `operate(edit)`, using the edit helpers of the types crate,
so a change wakes only the bindings that read what changed
(guides/reactive.md, Blocks). `editor.content_of::<C>(block)` is the same for
another block.

- Per-view state is a signal the component makes.
- State worth keeping for this viewer (scroll position, open folders) goes in
  the editor's view block: a `ViewStateKind` type read and written with
  `editor.view_state::<K>()`, on every change (guides/blocks/workspaces.md,
  View state).
- Work derived from signals is an effect over them. Something that moves with
  time is a beui timer that stops when idle. Work finishing on another thread
  wakes the editor through `editor.woken()`'s `Waker`.
- An editor keeping a mirrored copy (the text editor) calls `record()` on the
  projection and reads `take_projected()`: each foreign edit applied, or
  `Projected::Rebuilt` when it had to re-read the content.

### Gestures

An editor writes every change to its block as it happens (a stroke as it grows,
text as it is typed) so peers see it at once; the client batches saving. A
gesture that chooses one result on release, such as dragging a layer, which
nobody would expect kept half done, is tentative
(decisions/tentative_edits_wait_for_release.md): open it with
`ContentProjection::begin_tentative`, make its edits with `operate_in`, and end
it with `commit_gesture` or `cancel_gesture`. Its edits show at once in every
view of the block on this device and are sent and saved only when it commits;
a cancel, the instance closing or a crash leaves no trace. A tentative edit only
moves or changes what exists: anything a drop creates, create on commit.
(Moving today's drags, scrubs and the colour picker onto it is
plans/tentative_gestures.md, step 4.) That decision is about block content;
view state is written on every change whatever the gesture.

Bracket a gesture that writes as it goes with `begin_gesture` and
`commit_gesture` so it undoes as one step; `cancel_gesture` puts back what it
wrote and leaves no history. Each `begin_*` returns a `ContentGesture`, and the
gesture's edits go through `operate_in(gesture, edit)`; `operate` makes an edit
outside any gesture; an edit through a handle whose gesture is not open (it
ended, or began before the content arrived or while the block was not editable)
is dropped. An editor may hold several gestures at once (a second
finger, a second controller), each ending on its own. `break_step` ends the
editor's burst of quick edits so its next edit starts a step of its own, and
leaves open gestures alone. The host cancels every gesture still open when the
instance closes, so a gesture held open until something is decided (the
canvas's new text, kept only once written in) ends with its editor.

The replica may end a gesture itself: an undo while a tentative gesture is held
cancels it, as do a replacement of the content and a session restart that drops
what it moved. The projection then cancels the gesture for the editor, drops what
it makes in it after, and runs `ContentProjection::on_gesture_ended(run)` with
it: stop the gesture there (a drag lets go of what it carried), and end it as
you would anyway; ending it again sends nothing.

### What the host says

Unasked, the host's news arrives as signals on the `Editor`, written at the top
of the frame it was delivered for: `drag()`, `files()`, `audio()`,
`history(block)`, `focused_block()`, `peers::<P>()`, `editable()`,
`chrome_shown()`, `canvas()`/`scale()` and so on. An effect that reads one
runs again when it changes; nothing is polled. Answers to requests arrive
through `editor.then(asked, answered)` ([apps/host_services.md](apps/host_services.md)).

`editor.editable()` is the only thing keeping a read-only viewer from
editing: the host cannot grey out an app's surface, so every gesture that
would change the block checks it (`read_only()` is the memo a `disabled`
prop takes).

### Drawing the block itself

An editor that lists `Preview` among its manifest views writes
`BeuiRoot::Preview`, which the host asks for wherever it paints the block
rather than opening it: a canvas entity, a slide, a block embedded in text.
The host maps that screen onto the quad it paints, so Preview fills its
screen, has no background (what is behind the block shows through) and lets
the host place, rotate and fade it. It gets an `Editor` of its own, so its
children are placed apart from the main view's. A frame, unlike a preview, is
cleared to its `Root`'s `clear_color`.

### Examples

The counter editor is the smallest; checklist a larger retained document;
the deterministic game a creation dialog with a block picker; the
presentation children, playback and a preview; the pan-and-zoom example a
camera with nothing stored; `crates/be-app-example-triangle` an app with no UI
framework.

### Testing

`be_sdk_test::BeuiTest` runs the editor through the same app session the
shipped app runs (`be_sdk::headless`), against a stand-in host that holds each
block in an in-process type module as the host does. It drives the app with
the same `testing::Gui` as beui's own tests (click, drag, type_text, key,
...), prints the drive steps on failure, and reads back what the app asked of
the host. Its snapshot is what beui's renderer would hand the GPU, so no
driver changes it; GPU drawing behind `draw_gpu` (guides/beui/components.md,
Drawing with the gpu) is not in it, so a test of a GPU-drawn editor asserts on
the block and the chrome around it.

A test only records the children an editor places; how the host runs,
promotes and hands back children is tested against be-host in
`crates/be-host-tests` (guides/testing_a_gui.md, 6), which is where an editor
that places children is best tested.

## 4. The manifest

`manifest.json` is the app's single source of truth: the app reads its
identity from it and the host everything else, both through
`crates/be-sdk-manifest` (`manifest_from_json`), which also checks it. A bad
manifest is skipped and shown in the Apps debug window.

```json
{
  "id": "be3.foo",
  "name": "Foo",
  "version": "0.1.0",
  "editors": {
    "foo": {
      "id": "6f1c2e0a-93d4-4b7e-8a15-2c9e4f0b7d31",
      "edits": "00000000-0000-0000-0000-000000000000",
      "display_name": "Foo",
      "icon": "",
      "templates": {
        "main": {},
        "starter": {"name": "Starter foo", "icon": "", "category": "template"}
      },
      "views": ["Main"]
    }
  },
  "block_types": {
    "foo": {
      "id": "00000000-0000-0000-0000-000000000000",
      "display_name": "Foo",
      "icon": "",
      "color": [64, 155, 173],
      "module": "foo_types.wasm"
    }
  },
  "entry_point": "foo.wasm",
  "network": [],
  "extensions": []
}
```

The `be_sdk_manifest` types document every field. What is easy to get wrong:

- An editor's `id` is a fresh uuid. View blocks, default-editor settings and
  templates name the editor by it, so changing or sharing it breaks them.
- `edits` is a `CONTENT_TYPE` some manifest (or the host) declares; the host
  refuses an editor or template of an undeclared type. `"any"` edits every
  type (`crates/be-app-example-block-debug`, which reads the saved head with
  `editor.stored_head`). No `edits` makes a shell editor (the workspace, the
  desktop, Files, the picker): opened only on its own view block, never
  offered by Open with or the add menu, and refused if it declares templates.
- `icon` is a codepoint of the Lucide icon font, not a name.
- An editor with no `templates` is never in the add menu. Templates appear in
  the order written; `category` picks the menu tab.
- `accepts` and `opens` are for shell editors: which requests from editors
  inside it it handles (`show_block`, `dialog`, `panel`, `window`; `pick` and `rename`
  mark the editor picker and rename children run in),
  and which editors it places as soon as it opens.
- `network` lists the host names `fetch` may reach, matched whole against an
  `https://` URL's host, or `"*"` for any (still https only), which
  first-party apps use for now; `extensions` the protocol
  packages beyond the core it uses (`be_sdk_api::EXTENSIONS`, only
  `be3:desktop`). The host denies anything else.
- `"block_agents": true` lets the app make agents bound to one block
  (guides/apps/host_services.md, Asking and answering); the host denies those calls
  to any other app.
- `block_types` belong to the type, not an editor (adding_a_block.md, Declare
  it in the manifest).
- `workers` names the app's worker modules by file name (`"tiles.wasm"`),
  each a plain `.wasm` name other than the entry point (5, Workers).

The first editor found for a type is its built-in editor; others are offered
beside it (Open with, and the Default editors setting). Only the workspace's
tabs honour a chosen editor; embedded children open in the built-in one.

## 5. What an app can depend on

An app is one wasm for every platform, so it can depend on nothing that
reaches the network (`host.fetch` instead), opens a file (`read_data` for
staged data), or uses wasm-bindgen. It links against the shared libraries
(guides/app_libraries.md); `app(libraries = [...])` overrides that. A C or C++
library links in as a position-independent static library through
`app(extra_deps = [...])`, or a worker's `cargo_library(extra_deps = [...])`
(PDFium, `third-party/pdfium`, in crates/be-worker-pdf-render). `std::thread` works,
but only the app's first thread may call the GPU. An app has no fonts of its
own; the host sends them.

Data an app ships rather than keeps in a workspace goes in its `app()`'s
`data`, staged under `data/<app id>/` and read with `list_data`/`read_data`
(the games editor ships every game module this way).

### Workers

Work that should not hold up the app's frames (rendering a PDF page,
rasterizing map tiles) runs in a worker: an instance of a module of the app's
own, on a thread and isolate of its own natively and in a web worker of its own
in the browser, that the app talks to by messages (`be_sdk::worker`,
crates/be-sdk-worker; plans/single_threaded_guests.md). The map's
(crates/be-worker-map, be-worker-map-tiles) and the PDF editor's
(crates/be-worker-pdf, be-worker-pdf-render) are the examples to copy.

- `be-worker-<app>` holds the messages the app and its workers exchange,
  encoded however suits them (both examples use postcard, with `serde_bytes`
  for pixels), and the module names. The app depends on it alone: an app links
  whatever it depends on, so it must not depend on a worker's work.
- `be-worker-<app>-<worker>` is one worker: a `Work` (`start`, then `receive`
  for each message, answering through `App::send`), and beside its Cargo.toml
  a `worker.rs` that only calls `be_sdk_worker::worker!` on it, which
  `worker(module = "tiles")` from `buck/wasm/defs.bzl` links as `:module`
  (against the app's libraries unless `libraries` says; both examples need only
  the base library). A C library it needs goes in its `cargo_library`'s
  `extra_deps`, so the app never links it. crates/be-sdk-worker-echo is the
  smallest.
- The app names it in `app(workers = ["//crates/be-worker-foo-tiles:module"])`
  and in its manifest's `"workers": ["tiles.wasm"]`; the stage puts it under
  `workers/<app id>/` and refuses an app staged without a worker its manifest
  names.
- The app starts it with `Worker::spawn("tiles.wasm")`, sends it byte buffers
  with `send` (or a `Sender` from `sender()`) and awaits each answer with
  `receive`, one buffer a message, whole and in order. Dropping the `Worker`
  and its senders stops it; `receive` says why one stopped (it could not
  start, trapped or panicked).
- Work every editor of the app asks of goes to one worker they share, since a
  canvas can show more editors than the app may run workers: each editor holds
  a `Shared::new("tiles.wasm", wake)`, sends with `send` and takes its own
  answers with `try_receive` once `wake` is called. The worker starts with the
  first and stops with the last; one that stopped (a job it failed stops it)
  fails what each editor had in flight, and the next `send` starts it anew.
  Each message carries its client, so a `Work` answers whoever asked without
  the app's messages naming them. The map and the PDF editor work this way.
- A worker has WASI and its libraries but no gpu and no host services: it
  computes and sends back bytes, which the app uploads or shows. It may block
  its own thread as long as a job needs.
- An app may spawn only its own workers, named, never by a path, at most 16 at
  once with at most 1 GiB of memory each (natively: the browser cannot cap a
  worker's memory), and compiles no module from bytes (guides/apps/runtime.md,
  Backends). Its workers stop when it stops or fails.
- The app's tests run its workers in their own instance: the app
  dev-depends on the worker crate and the test calls
  `BeuiTest::serve_worker::<Tiles>("tiles.wasm")`, after which each job the
  editor sends is done before it next steps. A worker crate tests its `Work`
  through `be_sdk_worker::Hosted` (`ask`).

## 6. Build and verify

`./scripts/buck run //crates/be-host:native` builds and stages every app;
`//crates/be-host:apps` is the apps alone, `//crates/be-host:web` builds every
app as the browser's guest. To drive an app without a GUI:
`./scripts/buck run //crates/be-wasm-host:example-instantiate -- APP.wasm EDITOR_ID`
opens an instance, asks for a frame and fails unless it presented; every
library it links has to lie beside it as `lib-<SHA-256>.wasm`. Staging
details are in [apps/runtime.md](apps/runtime.md) and guides/buck2.md.

# The editor API

What a beui editor can do, by topic: its block's content, state, what the host tells it, the
graph, its frame and chrome, children, creating blocks, artifacts, presence, and the calls the
host answers. Read the section for the task at hand. Adding a new editor is
`guides/plugins/adding_an_editor.md`; the machinery underneath is `guides/plugins/architecture.md`.

## The `Editor` handle

- An editor's view is one `#[component]` function handed an `Editor`: a `Copy` handle that
  closures take with `move`.
- The instance keeps the `OwnedEditor` that owns its state, and frees it when the instance
  closes, after the views built on it.
- `editor.host()` is the `EditorHost`, for everything the plugin cannot do itself
  ([Host calls](#host-calls)).
- Nothing in an editor runs every frame. What the host says arrives as signals; projections and
  graph reads track, so an effect or memo that reads them runs again when they change.

## Reading and writing the block

`editor.block_content::<FooContent>()` is a `ContentProjection`. The framework pumps it at the top
of every frame inside the document's reactive scope.

- Register what the view reads as projections of the content: `project`, or `field`, `ids` and
  `object` for one field, one list's order or one object.
- Write back through `operate` with the edits the content type's helpers build. `operate` is
  already held behind whether the host says the block is editable.
- A change to the block wakes the bindings that read what changed rather than rebuilding the
  view. `guides/beui/reactive.md` says how projections work.
- An editor that keeps a copy of its own (the text editor mirrors its text) calls `record()` on
  the projection and reads `take_projected()`: each foreign edit the projection applied as it came
  (`Projected::Applied`) or a rebuild of its view (`Projected::Rebuilt`), after which it re-reads
  the content.
- `editor.content_of::<C>(block)` is the same projection for another block.
- `editor.raw_block_content()` reads the block without knowing its type: the content type and
  bytes of every snapshot the host sends. An editor using it asks for a fresh snapshot whenever
  operations arrive instead of applying them (the Debug view, `crates/editors/block-debug`).

## Editable and read-only

- `editor.editable()` is a signal, written before every frame, of whether the host says the block
  may be edited. `editor.read_only()` is the memo a `disabled` prop takes.
- The host cannot disable a plugin's surface the way it greys out an editor it draws itself, so
  an editor whose block may only be read keeps every gesture that would change it behind this.
  The pixel art editor still pans, zooms and reads out pixels, but draws nothing and disables the
  buttons that would.
- Check it before writing back what the editor worked out (an image's decoded size), so nothing
  is written on behalf of a reader.

## State

| Kind | Where it goes |
|---|---|
| local to this view | a signal the component makes |
| worth keeping for this viewer (scroll position, open folders) | the editor's view block: `editor.view_state(key)`, `editor.set_view_state(key, state)` |
| neither (a snapshot to recompute, a picker's answer to collect) | an effect over the signals it reads |

## What the host tells the editor

Each of these is a signal on the `Editor`, written at the top of the frame it was delivered for.

| Call | What it carries |
|---|---|
| `editable()`, `read_only()` | see above |
| `replies()`, `on_reply(...)` | answers to requests |
| `drag()` | the block the app is dragging over this screen ([Drag and drop](#drag-and-drop-from-the-host)) |
| `files()` | files dropped on the editor |
| `audio()` | playback state |
| `history(block)` | a block's history |
| `focused_block()` | the block the user is looking at |
| `web_view_events()` | what the editor's web views did |
| `peers::<P>()` | other peers' presence of kind `P` |
| `chrome_shown()` | whether the host shows this instance's chrome |
| `canvas()`, `scale()` | the camera of a pan-and-zoom editor |
| `presence_visible()`, `revealed()` | see [Presence](#presence) |
| `host_value::<T>()` | a host value ([Host values and actions](#host-values-and-actions)) |

## Time and work off the frame

- Something that moves with time, like playback, is a beui `create_timer` that stops once it has
  nothing left to do. `editor.audio()` changes only when playback starts, pauses, seeks, ends or
  fails, so the position it carries is where playback was then; the audio editor advances it on
  its own clock.
- Work finishing on another thread wakes the editor through the `Waker` from `editor.woken()`,
  whose signal an effect reads to collect the result. `host.waker()` hands out the same
  `block_editor_beui::Waker` for work with no ui to ask from.
- An editor doing work off the frame (the PDF editor renders pages on a thread of its own) can
  also ask to be drawn again with `ui.ctx().request_repaint()` or `request_repaint_after(...)`,
  which reaches the runtime's loop from any thread.
- An idle editor that asks for nothing costs nothing.

## The graph

A plugin has no connection, no block store and no key of its own: the app is the only peer, and a
plugin reaches blocks only through the host. The graph (which blocks exist, their names, parents,
references and access) is the host's mirror of the server's.

- `editor.watch_blocks(query)`: a memo of the `BlockInfo`s the host answers a `BlockQuery` with:
  `Roots`, `Detached`, `Children`, `References`, `Backrefs`, `Parents` or one `Block`.
- `editor.blocks()`: a `Blocks` handle.

| `Blocks` method | Does |
|---|---|
| `create`, `create_named`, `create_artifact`, `create_with` | make a block with its first content |
| `set_parent` | move a block |
| `set_name` | name one by hand |
| `define` | set any other defined metadata key |
| `info`, `access` | read what the host last said |
| `watch` | the query as a `BlockList`, without the memo |

`editor.create_child(&content)` is `create` under the editor's own block.

Each is a `GraphCommand` the session sends the host as `CreateBlock`, `SetParent` or `Define`. The
host applies it to its mirror straight away, checks the account may edit what it touches, and
hands it to the app's peer, which carries it to the server. Because the peer belongs to the app,
a block an instance has just created still reaches the server after that instance closes.

## Links, labels and icons

- `<BlockLink editor block>` links to another block, handed the `Editor` and an
  `Option<ChildTarget>`. It follows that block's name and icon and opens it in the host on click.
- `host.block_types()` is the host's registered block types, for naming and illustrating a block
  the editor only holds a reference to. Pass it to `block_ui::BlockLabel` (re-exported as
  `block_editor_beui::block_ui`) so labels match the rest of the app, including the italics of an
  automatic name.
- Icons come from `beui::icons` (a re-export of `beui_core::icons`), drawn with the Material
  Symbols font every beui context carries.

## The frame and its chrome

An editor gets the whole frame, and the framework draws nothing across it. The editor draws its
own chrome (toolbars, sidebars) inside it.

### Whether chrome is shown

- `editor.chrome_shown()` is a signal, written before every frame, of whether the host shows this
  instance's chrome at all. The same code draws a tab of its own, a block edited live inside a
  canvas and a slide inside a deck; this is how it tells them apart.
- An editor with a sidebar hides it when `chrome_shown()` reads false.
- `editor.content(&node_ref)` names, once, the node the content is in. The framework reports that
  node's rectangle after every layout. The host cannot see where an editor put its sidebar, so
  this is the rectangle it pans, zooms and reads wheel gestures against: a wheel over the sidebar
  is not a wheel over the content.

### `Sidebar`

`<Sidebar shown={editor.chrome_shown()}>` is the band the editors share (block_editor_beui):

- It takes the side it sits on, scrolls what is written between its tags, and draws the border
  against the content.
- It measures to nothing while `shown` is false, so its row gives the whole frame to the content.
- On a frame narrower than `NARROW_WIDTH` it leaves the row: its contents move into a `Sheet`
  under the content, opened by a toggle the framework adds to the editor's `Toolbar` (whose
  buttons then scroll sideways instead of wrapping), or to a bar of its own when there is none,
  or to the frame's menu on a phone. `sheet_control()` opens it from an editor's own button.

### Narrow frames and phones

- `narrow_chrome()` is the same narrow test, for an editor that wants its toolbar compact there.
  It is only the frame's width, which a desktop pane can have too.
- `phone_layout()` says the app itself is on a phone (the host asked for `TopBar::Phone`).
- Prefer one layout that works on both over one only for phones:
  - The canvas keeps its tools in a `BottomDock` over the stage, with the selection's tools in
    the row above it.
  - The text editor keeps its formatting in a bar that shows while the text has focus. Its
    buttons use `press_focus=false`, so focus and the keyboard stay in the text. Its button that
    puts the keyboard away shows only when a finger focused the text
    (`TextAreaState::touch_mode`), since only then is there an on-screen keyboard.
- The database, worktree, game module and audio editors are examples.

### Transparency

A frame is transparent wherever the editor does not paint, which lets the layer beneath show
through around a block edited inside a canvas. Do not wrap a pannable content area in an opaque
background (`guides/plugins/pan_and_zoom.md`, Staying transparent).

## The frame's menu and actions

The host asks for a menu with `FrameSpec::top_bar` (`TopBar::Shown` on desktop, `TopBar::Phone`
on a phone, `TopBar::Hidden` for none). The framework builds it; an editor never offers the menu
or the way out of its own frame, so it cannot lose them.

- It holds undo, redo, rename, share, the command palette, the editor's own commands, file
  details, and Close when the frame belongs to a child that took it over (Escape also leaves).
- The menu's commands are actions (`guides/beui/input.md`, actions and the command palette).
  Undo owns Ctrl+Z, Ctrl+Shift+Z and Ctrl+Y for the block while the host asks for the menu, after
  the focused control has had the key, which is how a text area keeps its own undo. Ctrl+Shift+P
  opens the frame's command palette over everything the editor registered.
- An editor adds commands by registering an action marked `in_menu()` while its view is built:

  ```rust
  Action::new(id, label, run).glyph(..).enabled(..).in_menu().register();
  ```

  The item goes when the scope that added it does. The canvas and the text editor offer the
  commands that do not fit their bars this way instead of a toolbar.
- Every action, in the menu or not, is in the command palette. Buttons and menu rows that run it
  take it as their `action` prop, so all share its label, enabled state and shortcut.
- File details asks the instance that placed the block, which hears it through `ChildBlock`'s
  `on_bar` (`BarAction::Details`).
- The rename and share entries (`rename_block`, `share_block`) are drawn by the shell
  ([Shell editors](#shell-editors)).

## Keyboard, back and the cursor

- The system back gesture and the Back key reach a plugin's `BackHandlers`. The host sends back
  to the last screen it drew that has one enabled, unless its own document handles back first,
  so an editor's open sheet closes before the workspace leaves the file.
- Shortcuts that must reach an instance whatever has the keyboard (a program's window, another
  plugin, the host's controls) are its intercepting actions (`guides/beui/input.md`). block-app
  honours them for the shell's frame only.
- A document with a focus-trapping layer open gets the keyboard from the host until the layer
  closes, then hands it back.
- The cursor the node under the pointer asks beui for is shown by the host while the pointer is
  over that screen, so a canvas can offer a crosshair or a grabbing hand.

How these cross the protocol is in `guides/plugins/architecture.md`, Window requests.

## Pointer lock

`unstyled::PointerLock` (`guides/beui/components.md`) asks the host to hold the pointer still and
hide it, for an editor that looks around a scene (`crates/editors/scene-3d`).

- The framework turns the host's `PointerMotion` and `Focus` events into beui's, so the control
  reads motion rather than a position and lets go on Escape or a lost focus.
- The host holds the pointer while any screen of any instance asks and lets go as soon as none
  does, so a closing instance cannot leave it trapped.

## Drag and drop from the host

- `drag()` is the block the app is dragging over the screen being drawn, in that screen's
  coordinates, and whether it has been let go. Draw the drop feedback yourself and answer
  `host.accept_drag(bool)`, which decides the cursor the host shows.
- A drag that starts and ends inside the editor's own document (a card between columns, a slot in
  a hotbar) never reaches the host: that is beui's `Draggable` and `DropTarget`
  (`guides/beui/components.md`).

## Files, network and data

- Pick files with beui's own picker (`guides/beui/components.md`, file picking). Inside a plugin
  beui forwards the request to the host's picker, so the same code works on desktop, Android, the
  browser and in a plugin. `block_editor_beui::FileChooser`, a `Copy` handle owned by the scope
  that made it, adds the import step, the chosen file's name, the error to show, and
  `on_chosen(...)` to take what was imported.
- `host.save_file(file)` saves a `SavedFile` (name, MIME type, bytes) through the platform's save
  dialog, or as a download in the browser. `block_editor_beui::FileSaver` wraps it; its `poll`
  answers saved, cancelled, or why it failed. The file tree's Export is the example.
- `host.fetch(url)` is a GET the host makes for the plugin, answered with `take_fetch(request)`
  once the body arrives or the attempt fails. A plugin has no network of its own on any platform.
  The host refuses any URL whose host name the manifest's `network` list does not name. The paint
  review editor downloads its snapshots this way.
- `host.list_data()` and `host.read_data(path)` read the read-only files the plugin was staged
  with, answered with `take_data_listing(request)` and `take_data(request)`. A plugin has no file
  system, so data it ships goes in the `data` of its BUCK `editor()` call, a map from a relative
  path to a source or target. The build stages it under `data/<plugin id>/` beside the app and the
  host reads it from there on every platform. The games editor ships every game module this way.

## The block picker

`host.pick_block(filter)` is how an editor acquires a child without waiting for one to be dragged
in. `block_editor_beui::BlockPicker` wraps it and answers with the block the user chose or
created.

- The filter names what is being chosen, the block types that may be chosen (empty for any), and
  whether the picker opens on the host's templates rather than its new-block grid.
- The host passes the request to the workspace, which draws the picker and answers it.
- A creation dialog may ask too: that is how an editor whose block cannot exist until it
  references another (the game editor, which plays a game module block) gets that block first.
  The workspace opens the picker over the dialog, and a block created there is created at the
  root.

## Presenting

`host.present(on)` asks for the whole window with no chrome bands and nothing else of the app;
`host.presenting()` reports what the host settled on. The host takes presenting away when the user
presses Escape or the instance stops being the one on screen, so read `presenting()` rather than
remembering what you asked for, and draw the content the same way either way.

## Web views

A web view of the operating system's own, laid over the app where the editor places it, is the
only browser a plugin has. The browser tab editor is the example.

1. Pick a `WebViewId` for each view.
2. Open it once with `host.open_web_view(id, url)`.
3. Place it with `<Subregion placed={Some(SubregionContent::WebView(id))}>`. Not placing it hides
   it.
4. Read what the page did with `editor.web_view_events()` (or `take_web_view_events(id)`); the
   browser tab turns them into the history its block keeps.

Also: `load_web_view(id, url)`, `reload_web_view(id)`, `focus_app(id)`, `close_web_view(id)`. The
host hides the view while the instance is not drawn, closes it with the instance, and on a
platform with no embedded browser answers with a failure the editor shows in its place.

## Placing things inside an editor

`<Subregion>` reserves a rectangle in the view and asks the host to fill it with what its `placed`
prop names, a `SubregionContent`:

| Variant | Content |
|---|---|
| `Block` | another block's editor (`<ChildBlock>` is this) |
| `WebView(id)` | one of the editor's web views |
| `Host(panel)` | a panel the host draws itself |
| `Window(id)` | a program's window |
| `Creation` | a template being made: the editor and template id from the catalog's templates |
| `ArtifactSettings` | the settings screen of a block's dynamic artifact |

Every kind shares the geometry, layering and status below.

- A `Creation` child reports progress in `ChildState::creation` (options, ready or not, then
  created or failed); `commit_child` asks the host to make it, under the parent and with the
  name given. The workspace's picker is built on it.
- `host.occlude(rect)` declares that the editor draws over that rectangle after the children
  placed so far, so the host neither draws nor routes input to them there.

### `ChildBlock`

`<ChildBlock>` puts another block's editor inside this one, several levels deep if need be. Every
editor is a plugin, so a child is always another plugin instance.

Props:

| Prop | Meaning |
|---|---|
| `editor` | the `Editor` this view was handed |
| `block` | the block and its type; `None` while the reference has not resolved |
| `mode` | `ChildMode` (below) |
| `layer` | `ChildLayer` (below; default `Below`) |
| `own_frame` | the child lays its own chrome out inside its rectangle |
| `top_bar` | with `own_frame`, the child offers its parent the frame's menu |
| `intrinsic` | the size you gave it; reaches the child as the size it was resized to |
| `rotation`, `opacity` | place a preview child at an angle or faded |
| `on_state` | reports `ChildState` to the editor around it |
| `on_view_change` | the child's pan, zoom and fit requests |
| `on_bar` | the child's bar actions (`BarAction::Details`) |

- It reserves its space with beui's `Embed` base node, which reports the rectangle and clip it
  was laid out in; the framework turns that into one placement per frame in the region drawn.
- The subtree between its tags is drawn inside that rectangle and is handed a `ChildBlockHandle`
  carrying a `ReadSignal<ChildState>`. A placeholder is a `Frame` whose visibility follows that
  signal, not a branch in the view. `handle.menu` is the child's frame menu, as actions that
  pick from the child.
- `on_state` is how a stage sizes itself to the slide's aspect ratio; `Frame`'s `aspect_ratio`
  prop fits a child of known shape into the space a stage was given.
- Place only the children that are visible: the host runs a bounded number as editors and draws
  the rest as previews.

### `ChildState`

What the host made of the child:

| Field | Meaning |
|---|---|
| `placed` | whether it is placed |
| `available`, `error` | false when the block cannot be opened: a block already open above this editor is refused, and `error` says so |
| `active` | whether the user promoted it |
| `intrinsic_size`, `aspect_ratio` | what the child reported |
| `interaction` | the child's manifest answer: run it live, or draw it as a preview until the user asks to edit it |
| `capabilities`, `resize` | what an editor laying children out in a world of its own needs before drawing a resize handle |
| `creation`, `settings` | progress of a `Creation` child; whether an `ArtifactSettings` copy changed |

### Modes

| `ChildMode` | Behaviour |
|---|---|
| `Passive` (default) | the editor keeps every click; nothing reaches the child |
| `Active` | the user promoted it (a double-click, an Edit button); the host takes it away on a click outside or Escape |
| `Live` | the whole of what the editor shows (the slide a presentation edits); live from placement, never taken away |
| `Preview` | the host draws the block's preview instead of running its editor, clipped to the placement (a filmstrip of thumbnails) |

### Layers

- A child is drawn under the plugin's own surface, and its space is cut out of it. Anything the
  editor draws after the child (a selection handle, a drag ghost, a menu, a modal) covers it by
  being drawn later.
- A genuinely translucent child is placed with `layer=ChildLayer::Above`, which composites against
  the plugin's pixels and cannot be drawn over.
- Children are placed in the order they paint. An overlay of the editor's (a menu, a window of
  its dock) covers the children painted beneath it, for drawing and the pointer, but not the
  children inside it.

### A child taking the frame over

A child in `Active` or `Live` mode takes the frame over:

- It is handed the same frame this instance was, draws every chrome band in it, and draws its
  content at the rectangle it was placed at.
- The editor that placed it draws no chrome while that lasts: its bands go away and its content
  takes the whole frame rather than keeping an empty band. Its camera is untouched, so the world
  and the child's rectangle in it stay where they were on screen.
- The part of the frame the child paints chrome over is withheld from the editor underneath, so a
  click on the child's toolbar or sidebar belongs to the child. A click on the rest, which the
  child leaves transparent, falls through to the editor that placed it, and that takes the frame
  back.
- There is no way to nest a child's chrome inside a band of your own, or to keep a band while a
  child has the frame. A deck whose slide is being edited puts what it wants on screen in its own
  content area, beside the stage.
- The user leaves with Escape or the exit the framework draws, and the host hands the frame back.

### A child with a frame of its own

`own_frame=true` gives the child its own chrome inside its rectangle instead: it draws its toolbar
and sidebars there, and the editor that placed it keeps its whole frame. With a `top_bar` too, it
offers its parent the frame's menu: every block tab in the workspace gets one this way, while the
pane of files does not. The workspace hands those menus to its dock with `dock_menu`, so the same
menu is behind the More button of the block's tab on desktop and of the stacked bar on a phone.

### A child's camera

A child with a view of its own hands its pan, zoom and fit requests back through
`on_view_change` rather than moving itself, so the editor that placed it decides what they mean:
a canvas turns a block asking to fit into fitting that block in the canvas
(`guides/plugins/pan_and_zoom.md`).

## Previews and embedded size

- An editor that lists `Preview` among its regions is asked to draw its block wherever the host
  paints it rather than opens it: a canvas entity, a slide, a block embedded in text. It writes
  `BeuiApp::preview_view`, which the framework builds a second document of, with a second
  `Editor` so the two regions place their children apart, the first time the host asks for that
  region.
- The host maps that screen onto the quad it paints, straight from the plugin's surface, so
  `preview_view` fills the screen it is given and lets the host place, rotate and fade it. It has
  no background of its own; whatever is behind the block shows through.
- Children in a preview work like children anywhere, so an editor whose block is only its
  children (a presentation showing its first slide) draws them there as previews.
- `BeuiApp::intrinsic_size`, or `editor.set_intrinsic_size` for one that changes, is the size the
  editor wants wherever it is embedded (a canvas, a text block). `BeuiApp::aspect_ratio` is the
  block's shape, which the host holds a preview to. Leave either unimplemented to take the host's
  default.

## Creating blocks

Every entry in the add-block menu is a template an editor's manifest declares
(`guides/plugins/adding_an_editor.md`, Templates), and the plugin makes every block from one.

- The host opens an instance with no block and calls `BeuiApp::create_block`, handed a `Creation`
  whose `template()` is the template id (`Creation::MAIN`, `"main"`, for the plain block). It
  answers with the new block's id, or why it could not be made.
- `creation.create(&content)` makes the block with its first content and leaves it `Detached`.
  The host adds the reference and sets the parent once it has the id, then opens the block, so
  one plugin runtime serves the creation and the editor that follows.
- The block and its content travel together in one `CreateBlock`; the app's peer creates the
  block, seeds its first commit and only then sets the parent, so `create_block` waits for
  nothing.
- A non-dialog template is asked for its block as soon as the instance is open; the host shows
  only a spinner while it starts.

### Dialog templates

A template whose block cannot exist until the user has chosen something (the file behind an
image) sets `"dialog": true`, and its editor writes `creation_view`, a `#[component]` the framework
draws inside the picker's creation dialog.

- It says what the dialog makes with `creation.on_create(...)` rather than overriding
  `create_block`.
- `creation.set_ready(true)` (on the host, `set_creation_ready`) tells the host the dialog has
  been filled in and may be accepted.
- A dialog creation instance gets the dialog's whole plugin rectangle through `creation_frame`.
- `content_file_creation::<C>` is the `creation_view` of a dialog whose block is a file the user
  picks: it shows the file picker and creates the block from what the import function makes of
  the file.

## Artifacts

A block type that produces dynamic artifacts (a PNG exported from a drawing) lists
`ArtifactSettings` among its regions, which tells the host this plugin answers for its blocks'
artifacts. The pixel art export and compiled logic are the examples.

1. The editor creates one with `editor.blocks().create_artifact(&content, parent, name, source)`.
   Its `ArtifactSource` names the source's type and carries an opaque payload for the plugin,
   kept in the artifact's sealed metadata. It then asks the host to open it.
2. While that artifact is on screen the host keeps a second instance of the plugin open and asks
   it, through `connect_artifact`, `describe_artifact` and `artifact_settings_view`, and the
   `Artifacts` handle's `on_regenerate` and `on_poll`:
   - which block the payload was generated from and what it currently produces;
   - how to edit it in the workspace's settings dialog (a `Subregion` of `ArtifactSettings`,
     whose `ChildState::settings` says whether the copy changed; committing that child applies
     it);
   - how to rebuild it.
3. The host owns the stored settings and hands them over whenever they change. The settings
   screen edits a copy, which the host stores only once the user applies it and throws away when
   the dialog is dismissed.
4. Regeneration reads its source with `host.content_of::<C>(source)`, a projection owned by the
   `OwnedContentProjection` it returns, which turns loaded once the host has sent it, and writes
   the result with `host.replace_content(artifact, &content)`. So `on_poll` answers `None` until
   the source has arrived.

## Presence

- `editor.show(Some(&value))` publishes a collaborative cursor, a value of a
  `be_block::presence::PresenceKind`. `host.show_presence` does the same for a block other than
  the editor's own.
- `editor.peers::<P>()` is a signal of every other peer's value of that kind.
- `presence_visible()` is a signal, written before every frame, of whether anyone is looking at
  this block, which is when a cursor is worth publishing.
- `revealed()` asks the editor to scroll one of those cursors into view; only the editor knows
  where in its content that is.
- The host carries presence to the app's peer, which shares it with the block's session and never
  saves it, and takes it away when the instance closes.
- The framework shows a `UserActive` entry of its own while the block is visible. A peer never
  sees what it showed itself, so the presence an editor reads is only other people's.
- A cursor carries its own colour, picked with `pick_free_color` against the colours the other
  peers show.

## Replaced children

`editor.on_replace_child(...)` answers when the host copied a block because it was being edited
in two places at once.

- An editor whose references live inside its own content (the text editor's block URLs) rewrites
  them and answers `true`; one that leaves it to the block's own structural edit answers `false`.
- The callback runs outside the document's reactive scope, so it edits the block and leaves the
  view to catch up on the next frame rather than writing signals.

## Pan and zoom

A manifest with `capabilities.pan_and_zoom` gets a camera from the host: `editor.canvas()`,
`editor.scale()`, and `editor.pan`, `zoom`, `zoom_at`, `fit` and `reveal` to ask it to move.
`guides/plugins/pan_and_zoom.md` covers it.

## Host values and actions

What only the host can see and do: the programs' windows, input devices, displays, power, sound
and notifications on Linux.

- `editor.host_value::<T>()` is a memo of a host value, `T::Value::default()` until the host says
  otherwise.
- `editor.act(action)` sends a host action.

### Values

A host value is a marker type implementing `block_plugin_api::HostValue`: a `KEY` and a serde
`Value`. All are in block-plugin-api's `linux.rs` except `LinkBase`, in its `host_value.rs`.

| Value | Carries |
|---|---|
| `HostWindows` | the programs' windows with titles, parents and drawn sizes ([Program windows](#program-windows)) |
| `InputDevices` | input devices |
| `Displays` | displays |
| `Power` | the `PowerAvailability` the session allows |
| `Media` | `MediaLevels` of the default output and input and the screen's brightness, each `None` while unreachable |
| `Notifications` | the `NotificationInbox` of D-Bus requests the desktop has not taken yet |
| `ScreenLocked` | whether the screen is locked |
| `Programs` | the `.desktop` programs, each a `HostProgram` whose icon is a `HostImage` (plain RGBA) at the size last asked for |
| `LinkBase` | the URL of the server the workspace is on, which block links start with; empty for the device's own server (`be_block::block_url`) |

### Actions

An action implements `HostAction` (a `KEY` and serde).

| Action | Does |
|---|---|
| `WindowAction` | focus, fullscreen or close a window |
| `PowerAction` | power |
| `MediaRequest` | `StepVolume`, `SetVolume` (0 to 1), `ToggleMute`, `ToggleMicMute`, `StepBrightness`, or `Player` with a `PlayerCommand` for the player that played last |
| `NotificationReport` | the last request taken, the signals to send on the bus, and the ids still held |
| `ProgramAction` | `List { icon_size }` to read the programs again, `Launch` one by id, `Run` a command line |

### How they travel

- The first read of `host_value` sends `WatchHostValue { key }`. The host answers with
  `HostValue { key, value }` at once, whenever the value changes, and again after the plugin
  restarts.
- `act` sends `HostAction { key, action }`.
- Both cross the protocol as `encode_host` bytes, so the protocol names no feature.
- The `EditorHost` keeps a revision per key, bumped only when that key's bytes change, and each
  memo reads its own key's revision, so a push of one value neither wakes nor decodes another's
  watchers.

### On the host

- `plugin_host::publish::<T>(&value)` sets a value and tells watching instances when its
  encoding changed.
- `plugin_host::watched::<T>()` says whether any instance watches (the host starts PulseAudio only
  then).
- `plugin_host::take_actions::<A>()` takes what instances asked.
- Who may watch or ask is decided in one place: the `VALUES` and `ACTIONS` tables in block-app's
  `plugin_host/host_values.rs`. Each key is open to anyone or to the shell alone (the instance
  drawing the app's window, which `main.rs` names with `PluginEditor::claim_shell`). A key missing
  from its table is refused, an action from an instance that may not send it is refused as it
  arrives, and a value reaches only instances allowed to see it.

### Adding one

1. Its types and marker impl in block-plugin-api.
2. A row in `VALUES` or `ACTIONS`.
3. A `publish` or `take_actions` call where the host learns or carries it out.
4. `host_value` or `act` in the editor.

A test stands in for the host with `BeuiTest`'s `set_host_value::<T>`, `watches::<T>`,
`actions::<A>` and `take_actions::<A>`, or sets a value on the `EditorHost` with
`set_host_value::<T>`.

## Shell editors

The shell is the instance that draws the app's window (the workspace, or linux-desktop). It
declares `accepts` in its manifest (`guides/plugins/adding_an_editor.md`) and uses host calls no
other editor needs.

### Shell-only host calls

| Call | Use |
|---|---|
| `take_show_requests()` | every block anything asks to open, including the shell's own requests, so opening is one path |
| `report_focus(focused)` | the block the user is looking at, which the host tells every other instance |
| `take_panel_requests()` | host panels the app menu asks to show |
| `take_pick_requests()`, `answer_pick(pick, answer)` | block picker requests to draw and answer |
| `take_dialog_requests()` | rename and share dialogs any editor's menu asks for, as `ShellDialog`s |
| `commit_child(child, parent, name)` | make a `Creation` child |
| `list_access(block)`, `take_access_listing(request)`, `set_access(block, account, access)` | the share dialog: who has access |
| `watch_artifacts(blocks)`, `artifact(block)` | the blocks whose dynamic artifacts it shows, and what the host's artifact instances describe |
| `regenerate_artifact(block)`, `unlink_artifact(block)` | act on an artifact |
| `simulate_access(block, level)` | hold a block it is not drawing to less access than the account has |
| `close_editor(block)` | close the editor of a tab that was closed |
| `show_app_menu` | open the app menu |

### Docks and host panels

- An editor whose layout is a dock of tabs builds it with beui's `Docking` inside its own frame
  (`guides/beui/components.md`, docking). The dock and any window popped out of it live inside
  the plugin.
- The host's debugging panels (`HostPanel`: the block stack, performance, plugins and version) are
  drawn by the host but placed by the workspace. The app menu asks the shell to show one
  (`take_panel_requests`), and the workspace declares it as a dock window holding
  `<Subregion placed={Some(SubregionContent::Host(panel))}>`. The host knows a panel is wanted
  while some plugin places it.

### Program windows

On Linux the app is a Wayland compositor (`crates/be-wayland`, glued in by
`crates/block-app/src/wayland.rs`), and `editor.host_value::<HostWindows>()` lists its windows.
The host draws a window only for the instance that draws the app's window.

- Placing: the workspace gives each window a tab holding
  `<Subregion placed={Some(SubregionContent::Window(id))}>`, which the host draws and sizes the
  program to. A window with a parent becomes a `DockDialog` attached to the parent's tab
  (block-shell's `AttachedWindow`). linux-desktop floats its top windows in dock windows instead
  of tabs.
- Modal: a window the program marks modal (xdg-dialog-v1's `set_modal`) is listed with `modal`
  set, and its dialog is modal to its parent. The host keeps presses and keys from the parent on
  its side too, giving the keyboard to the innermost modal dialog; the dialog's window tab asks
  for the keyboard (`WindowAction::Focus`) when a press lands on the parent in the plugin.
- Closing: closing the tab asks the program to close (`WindowAction::Close`); the tab goes when
  the program's window does.
- Focus: a listed window is `focused` while the program has the keyboard, and the window tab then
  shows itself in the dock, so the dock's recent tabs follow the keyboard. When the dock focuses a
  window's tab some other way (a click on its bar, a commit of the switcher) the tab asks the host
  to give the program the keyboard (`WindowAction::Focus`), which also raises it.
- Raising: `raised` counts the times the host brought the window forward (`WindowAction::Focus`, a
  program's xdg-activation request, going fullscreen). The workspace shows the window's tab each
  time it goes up, even while hidden behind another tab whose panel was never built, and the
  program gets the keyboard once its tab is drawn. Raising a window that has a modal dialog
  raises the dialog instead, and raising a dialog shows the tab it hangs from.
- Fullscreen: a window that wants fullscreen (the program asked, or linux-desktop's Super+F
  toggled the focused one with `WindowAction::Fullscreen`) is listed with `fullscreen` set to the
  rect of its screen, in the plugin's coordinates. The window tab then makes its dock tab
  fullscreen over that rect (`guides/beui/components.md`, docking); if the dock lets go first,
  the tab asks the host to leave fullscreen (`WindowAction::Fullscreen`).
- Maximize only tells the program it is maximized; it already fills its tab.
- Super-drag: linux-desktop gives its dock `drag_modifier` Super, so Super+drag anywhere on a
  floating window moves it and Super+right-drag resizes it, or moves the split beside a docked
  one. The press has to reach the plugin rather than the program under it: block-editor-beui
  reports the document's `press_claims` with every frame (`Frame::claims`, `FrameReport::claims`
  in the protocol), and the host's `PluginRegion` claims those presses with `claim_at`, which
  also keeps them from any child editor there, a program's window among them (each window is a
  forwarding catcher like a plugin's region; `guides/beui/input.md`, the one input path).
- A client's own `xdg_toplevel.move` and `resize` requests (dragging a client-side titlebar) are
  not honoured yet.

## Host calls

Everything on `EditorHost` (`editor.host()`) not covered above by an `Editor` method, in one
place.

| Call | Does |
|---|---|
| `open_block(id, block_type)` | open another block in a tab of its own |
| `open_block_with(id, block_type, editor)` | the same in a chosen editor |
| `block_types()` | the host's registered block types |
| `editable()` | whether this instance's block may be edited |
| `drag()`, `accept_drag(bool)` | a host drag over this screen, and the answer |
| `save_file(file)` | save a `SavedFile` |
| `fetch(url)`, `take_fetch(request)` | a GET through the host |
| `list_data()`, `read_data(path)`, `take_data_listing(request)`, `take_data(request)` | staged read-only files |
| `pick_block(filter)` | the block picker |
| `present(on)`, `presenting()` | take the whole window |
| `open_web_view`, `load_web_view`, `reload_web_view`, `focus_app`, `close_web_view`, `take_web_view_events` | web views |
| `occlude(rect)` | cover children placed so far |
| `set_creation_ready(bool)` | a creation dialog may be accepted |
| `rename_block`, `share_block` | ask the shell for those dialogs |
| `show_presence(...)` | presence on another block |
| `content_of::<C>(block)`, `replace_content(block, &content)` | read and replace a block's content outside a view (artifacts) |
| `waker()` | a `Waker` for off-frame work |
| `view()`, `view_scale()`, `pan_view`, `zoom_view`, `fit_view` | the camera, for a plugin without beui (`guides/plugins/pan_and_zoom.md`) |
| `set_host_value::<T>` | set a host value (tests) |

Shell-only calls are under [Shell editors](#shell-editors).

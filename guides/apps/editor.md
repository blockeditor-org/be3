# Inside an editor's frame

What an editor draws and how the host's chrome, camera and dialogs fit around
it. [../adding_an_app.md](../adding_an_app.md) is the how-to; this covers the
parts that span many editors.

## The frame and its menu

An editor gets the whole frame and the framework draws nothing across it. When
the host asks for a top bar (`FrameSpec::top_bar`: `Shown` on a desktop,
`Phone`, `Hidden`), the framework sends the host the frame's menu: undo, redo,
rename, share, the command palette, the editor's own commands, file details,
and Close for a child that took the frame over. An editor draws no undo, redo
or exit buttons of its own.

The menu travels as a tree of `MenuEntry` (flattened on the wire with a
depth); the framework sends the whole tree again when any entry changes and
runs the command whose id comes back. The protocol knows nothing of beui.

An editor adds its part while its view is built:
`add_to_menu(vec![MenuNode::action(&action), MenuNode::submenu(..), ..])`
over actions it registered; the part goes when the adding scope does. Every
action is also in the command palette, and buttons and menu rows that take it
as their `action` prop share its label, enabled state, tick and shortcut
(guides/beui_keyboard.md, Actions and the command palette). An action with
`.pin(Pin::Icon | Pin::Label)` sits in the tab bar beside More; `Pin::Title`
runs when a phone's stacked bar's title is tapped (Rename). The canvas builds
its frame menu and context menu from the same submenus (`root/menu.rs`); the
text editor offers what does not fit its bar there.

The placer reads a child's menu as `ChildBlock`'s `handle.menu`. The
workspace hands it to its dock with `dock_menu`, adding what only it knows
(References, Referenced by, Access) before File details, so the same menu is
behind a tab's More on a desktop and the stacked bar on a phone.

Keys: undo owns Ctrl+Z, Ctrl+Shift+Z and Ctrl+Y for the block only after the
focused control has had the key (so a text area keeps its own undo).
Ctrl+Shift+P opens the command palette: the host's when `open-block` says
`host-palette` (be-host's always does; see Actions and the palette below),
whose menu entry asks for it with `shell.open-palette`, otherwise the frame's
own, as in app tests. Intercepting actions reach an
instance whatever has the keyboard (guides/beui_keyboard.md); be-host honours
them for the shell's frame only. The system Back gesture reaches an app's
`on_back` handlers: the host sends it to the last screen it drew that has one,
so an editor's open sheet closes before the workspace leaves the file.

A pick, and `use_frame_anchor()`, carry the rectangle the menu belongs to (the
tab, or the child's rect on a canvas), so a dialog opened from the menu, a
shortcut or the palette with `DialogAnchor::Rect` points at its tab.

## Chrome and side panels

An editor draws its own chrome inside the frame. `editor.chrome_shown()` says
whether the host is showing this instance's chrome at all; a sidebar hides
when it reads false. That keeps one code path right for a tab, a block edited
live inside a canvas and a slide inside a deck.

Name the node your content is in once with `editor.content(&node_ref)`. The
framework reports its rectangle after every layout; the host pans, zooms and
reads wheel gestures against it, so a wheel over a sidebar is not a wheel
over the content.

Side panels go in the editor's own dock (guides/beui/docking.md): a locked
panel pane beside a bare locked stage pane, under a `Show` of
`chrome_shown()`, sized with `SplitSize::Second(SIDE_PANEL_WIDTH)` so the
stage takes what the frame gains. The canvas splits its stage with Inspector
above Layers; the deterministic game puts a locked History pane beside its
board and its seat picker in a `DockBar`. Database, worktree, game module and
audio are further examples. When the editor's dock joins the workspace's
([children.md](children.md#docks-across-apps)) these panes become the root's.

## Phones

Below `NARROW_WIDTH` (or short and narrow in landscape) the workspace's dock
stacks (`DockMode::Stacked`): one screen at a time under the dock's stacked
bar. An editor's own dock keeps its stage and gathers every other pane's tabs
into one foldable pane below it (beside it when wider than tall), so an editor
writes nothing for phones. `DockingLayout::show` brings a folded pane
forward, as the canvas's selection bar does with the Inspector.

Prefer one layout for both. The canvas keeps tools and zoom in `DockBar`s the
user can move to any edge or window. `be_sdk_beui::Toolbar` is for a band
outside a dock (the file tree's); its buttons scroll sideways on a narrow
frame. `narrow_chrome()` tests the frame's width, which a desktop pane can be
too; `phone_layout()` says the host itself is on a phone.

## Pan and zoom

`capabilities.pan_and_zoom` means the content has a view of its own. The host
owns that camera (guides/pan_and_zoom.md;
decisions/pan_and_zoom_camera_belongs_to_the_host.md): `editor.canvas()` is
the `CanvasView` a beui `Canvas` takes and `editor.scale()` the zoom, both
written before every frame. The editor keeps no zoom or offset of its own and
asks to move the view with `editor.pan`, `zoom`, `reveal`, `fit` and
`keep_fitted`.

The host answers against whatever shows the instance, so the same controls
zoom a tab or, inside an infinite canvas, the canvas around it. A zoom readout
is true either way as long as it is read from the view it was given.

A `Canvas` places items; it does not transform them, so text inside keeps its
size. An editor whose content should grow with the zoom sizes its text by the
scale. A `Canvas` never told its view fills its space at scale one.

Wheel, pinch and middle- or space-drag over the content belong to the host,
read once for the outermost view under the pointer (a wheel over a block in a
canvas moves the canvas). `capabilities.pan` picks the gestures: `TwoFinger`
(default), `None`, or `OneFinger`, which also pans on a plain drag: the press
goes to the editor, which calls `editor.pan_drag()` when it pressed on nothing
of its own, and the host then cancels the pointer in the editor and pans with
no round trip. The map, game board and PDF editor do this.

## Making a new block

Every entry in the add-block menu is a manifest template, and the app makes
the block in `BeuiRoot::create_block`, handed a `Creation` whose `template()`
is the template id (`Creation::MAIN` for the plain block). A template may make
another type than its editor's (the presentation's slide templates make
canvases) and says so in the manifest.

The host opens an instance with no block; `creation.create(&content)` makes
the block Detached with its first content in one `CreateBlock`, and the host
then sets the parent, adds the reference and opens it in the same runtime.
`create_block` waits on nothing.

A template whose block cannot exist until the user chooses something sets
`"dialog": true` and writes `BeuiRoot::Creation`: a styled `DialogBody` with
its title, content, Cancel and a Create that calls `creation.accept()`;
`on_dismiss` calls `creation.cancel()` and also receives Escape, Back and a
press outside (decisions/app_dialogs_are_drawn_by_the_app_with_dialog_body.md).
It says what to make with `creation.on_create(..)`. `content_file_creation::<C>`
is the whole dialog for a block made from a picked file. "Import a file…"
makes a block with no editor at all, by the type module that claims the file
(guides/blocks/host.md, Exporting and importing).

## Artifacts

A type that produces dynamic artifacts (a PNG exported from a drawing) lists
`ArtifactSettings` among its views. The editor creates one with
`editor.blocks().create_artifact(..)`, pointing at the source block and
carrying the app's settings as an opaque payload. The host keeps a second
instance of the app open while the artifact is shown and asks it, through
`connect_artifact`, `describe_artifact`, the `ArtifactSettings` component and
the `Artifacts` handle (`source`, `on_regenerate`, `on_poll`), what the
settings produce and how to rebuild it. The settings component edits a copy
inside a `DialogBody` with Cancel and Apply that be-sdk-beui provides; the
host stores the settings only on Apply. Regeneration reads its source as a
projection, so `on_poll` answers `None` until the source arrives. Compiled
logic is the example.

## Presence

`presence_visible()` says whether anyone is looking at the block, which is
when a collaborative cursor is worth publishing; `revealed()` asks the editor
to scroll a peer's cursor into view. `editor.show(Some(&value))` publishes a
`be_block::presence::PresenceKind` value through the replica (never saved,
removed when the instance closes); `editor.peers::<P>()` is everyone else's.
A peer never sees its own, so an editor never draws a cursor for the person
at the keyboard. Pick colours with `pick_free_color`.

`on_replace_child` answers a block the host copied because it was edited in
two places at once: an editor whose references live in its content (the text
editor's block URLs) rewrites them and answers true.

beui has no IME output yet, so a beui editor cannot report an IME area.

## Actions and the palette

Each composited view's `FrameReport` carries `actions`, the
`described-action`s of the actions registered in its window (the main view
also those registered outside any window): id, label, glyph name, first
shortcut as a `key-chord`, enabled, checked, and `live` (answering where the
focus is). It is `none` when nothing changed since the view's last report, and
is sent whether or not anything asks the app to describe: be-sdk-beui-platform
follows the document's `PublishedActions`, which an effect in beui's action
registry keeps (`Context::document_actions`), so a label or enabled state
changing sends the list again with no per-frame comparison. The host keeps
each view's list while it asks the app to draw that view (the main view's
always), offers a window's or dock view's only while it shows it and an
anchored view's while it is open, and runs one with `app.run-action(screen, id)`, which
the guest resolves through `request_action_in` and runs only if still
enabled. `BeuiTest::described_actions` and `run_action` do what the host does.

be-host draws the one palette (`be-host/src/compositor/palette.rs`, Ctrl+Shift+P
in a window capture key listener that leaves the chord to a focused Wayland
window and, pressed again, closes the palette; or `shell.open-palette` from an
instance that shows a view and has the keyboard or whose frame menu was picked
from in its last two layout commits, which reaches the palette through its own signal,
`compositor::palette_asks`): the live actions of the instance whose view has
the focus, then of the instance that
placed it (`Catcher::placer`), out to the root, each under its block's name,
every shown view's of an instance with the placing view's first. Ids repeat across instances (each
has its Undo; the innermost lists first), never within one. Choosing one gives
the focus back to that view and runs it in the view it came from. The host's own
live actions follow (switch account, lock, the theme, which cycles the
display settings' theme through the system's, light and dark, written as the
Settings app writes it and applied by `sync_ui_settings`). A containing app's
chords do not reach it while a child has the focus
(plans/deferred/command_palette_shortcuts.md).

`open-palette(instance, blocks)` is quick open: the workspace's Ctrl+P (while
the workspace itself has the focus) asks for it, and the host lists blocks to
open from a `BlockSource` (`palette.rs`): handed quick open's query and which
opening this is (0 while the palette lists actions), it answers a memo of blocks already filtered and ordered, which the
palette keeps (`PaletteItem::ranked`). The default, `loaded_blocks`, reads the
graph's placed, viewable blocks of a type an editor shows each time quick
open opens (open ones first) and matches them fuzzily; plans/search.md swaps in
search hits. The workspace's other commands (Settings, Show files, New file) are its
own actions, and the tab keys and Split right, Split down and Close tab are
beui-dock-host's (guides/beui/docking.md).

## Accessibility

While a screen reader or a driver (guides/running/drive.md) is attached, the
host calls the app's `describe(true)`, and each composited view's
`FrameReport` carries an incremental AccessKit tree in the view's points and
the editor's own node ids. be-sdk-beui-platform fills it from the document's
tree, so a beui editor needs only the labels and `@test_id`s it already has;
another framework fills `Frame::description` itself. The host renumbers the
nodes into its own tree, repairs whatever the app sent, shows each view as a
pane named after the app with its actions, and calls back `node-action` and
`run-action`.

## Holding the pointer and sound

`unstyled::PointerLock` (guides/beui/components.md) asks the host (`WindowRequests::pointer_locked`) to
hide and hold the pointer while the editor reads motion (the scene_3d
editor). The host holds it while any screen asks, so a closed instance cannot
leave it trapped.

`editor.audio()` changes only when playback starts, pauses, seeks, ends or
fails; the editor advances the position on its own clock. The host decodes
only by media type, so an editor loads its bytes again after its content
changes.

# Overlays and layers

Everything that floats over a document. The machinery is in beui-core
(`layer.rs` for the layer list and stacking, `focus_scope.rs`, `escape.rs` for
layers that leave their window, `base/overlay.rs` for layout and placement);
the shared enums (`LayerPolicy`, `LayerLevel`, `Placement`, `OverlayAnchor`,
`Dismiss`, `LayerSpace`, `EscapedLayer`) are in `beui_types::layer`; the
`Overlay` component and `use_overlay_presence` are in
`beui-view/src/components/overlay.rs`. Components built on it (`Modal`,
`Dialog`, `Toasts`, `BackSlide`, `Floating`, `Tree`, menus, popovers) are in
beui-components-unstyled with faces in beui-components-styled. Tests are in
`crates/beui-host/src/tests/`.

## The layer tree

Every open overlay is a layer. A layer's parent is the layer holding the node
it hangs from (its anchor node if it has one, else the overlay node itself);
the document root is the bottom layer. `Document::layers()` is that tree as one
list, bottom to top, recomputed only when the tree or the layers change. Paint,
hit testing, focus, Escape, back and key dispatch all walk it.

Stacking:

- Children sit above their parent and rise with it.
- Among siblings, layers that block everything sit above those that don't;
  otherwise they are in the order they were opened or raised
  (`Document::raise_overlay`). Raising a window therefore never lifts it over
  a dialog.
- `LayerLevel` puts a layer and everything above it over every layer of a lower
  level: `Raised` (a dock's floating tool bars; a `Normal` layer that blocks
  everything counts as `Raised`, see `LayerLevel::stacked`, so menus and modals
  still open over those bars), `Top` (drag preview, volume OSD, toasts, so a
  toast stays pressable while a dialog is open), `Lock` (the lock screen).
- Opening a layer raises it and every layer it hangs in, so a menu opened in a
  covered window brings that window up. An `Inert` layer rises alone, so a
  tooltip leaves its window where it was.

## Anchors and placement

An overlay is anchored (`OverlayAnchor`) to a node, a point, a rect, a
`Pinned` rect that follows a node as it moves, or a `Band`: the node pushed out
to a band's edges on the axis the placement opens along (`layer::banded`), so
a button's panel in a bar opens just past the whole bar while lining up with
the button.

`BelowStart` opens below its anchor, or above when only that side has room,
and when neither side has room takes the roomier one and shrinks (scrolling
what it holds), so it never covers its anchor: completions, tooltips and
popovers keep their input or button in view. `Placement::Menu`, which menus
(`MenuPopup`, context menus) and selects use, instead covers its anchor when
neither side has room, as low as the window allows, and shrinks only when it
is taller than the window (`layer::resolve_rect`, `layer::room_beside`). A
menu that must keep clear of what opened it passes `placement=BelowStart` to
its `MenuButton` or `ContextMenu`: a dock bar's grip and the desktop bar's own
menus (tray, layouts, window chips), which must not hide the bar. The
HTML core places a `Menu` from its measured size, since CSS anchor positioning
cannot cover the anchor; so unlike a `BelowStart`, which CSS keeps on its
anchor, an open menu there moves only when beui lays out again, not as a DOM
scroller under its anchor scrolls.

`bounds` (window coordinates) narrows the room an overlay is placed, flipped,
shifted and shrunk in; a menu passes it to its submenus, and a panel that must
not cover a dock's bands takes the dock's area. The HTML core ignores it, and
fits menus with CSS anchor fallbacks and a `max-height` from
`layer::room_beside`.

`Placement::Popover` centres content `gap` below the anchor and can point a
beak at it. The beak lies outside the content's frame and takes no room, so
give it room with padding: an app's host draws whatever lies outside an
overlay's content under the editors it holds.

### Overlays stay in their window

A document knows nothing of monitors. Every overlay is laid out within the
window it was opened in (less any on-screen keyboard): `Center` and `Fill`
take that area, anchored placements are kept inside it, and a `Center` overlay
is never larger than its window. Where a program shows one window per monitor
(be-host's login session), a dialog stays on its monitor; be-host's
keep-changes prompt and lock screen are one `Modal` per display window.
`shown_in` moves a non-window overlay into another of the document's windows
(`Document::window_of` counts it there). `use_viewport_size()` is the size of
the window the caller is built in, which is what makes a `Dialog` a sheet in a
narrow app region. The exception is a window with a layer space, below.

## Content and lifetime

An `Overlay`'s content is built on open and removed on close, like `Show`: a
closed menu or tooltip costs only its overlay node and nothing in it is
focusable, hit, or described. `keep_alive=true` builds it on first open and
keeps it, with its state, while closed (dialogs, popovers, dock windows).
Content can be built more than once, so it is an `Fn`; a component holding a
`Render` it may call only once passes `render.once()` to a `keep_alive`
overlay. State the rest of a component reads (which rows a select's search
hides) belongs outside the content, derived from data, since the content's
nodes may not exist.

An animated close belongs to the overlay, not its owner: the content calls
`use_overlay_presence()` and hands `linger_while` a memo that stays true until
its exit finishes. The owner just writes `open` false. `ModalSheet` and `Modal`
leave this way.

## Policies

Each layer has one `LayerPolicy`:

- `BlocksAll` (default; menus, selects, dialogs): a press anywhere outside is
  shut out (it asks the layer to close and goes no further) and nothing beneath
  is hovered. One that draws a scrim or sits at `LayerLevel::Lock` is also
  modal to assistive technology (`Layer::modal`, `Document::inert`): what it
  covers is hidden, actions aimed into it are refused, and an `Embed` beneath
  hears `EmbedSlot::on_inert` so its host leaves that app out of its
  description. A menu has no scrim, so it isn't modal to assistive technology.
- `Pass` (windows, toasts, popovers): its content takes the pointer where it
  is; everything around stays live.
- `BlocksParent`: modal to the node it hangs from alone. A press there is shut
  out and asks the layer to close, focus can't enter that node, and assistive
  technology skips it. A modal `DockDialog` is one.
- `Inert` (tooltips, drag previews): takes no input.

`traps_focus` (default on) keeps Tab inside and makes the layer a key boundary
(guides/beui_keyboard.md). Menus, dialogs and popovers trap; windows don't. A
blocking layer that doesn't trap (the touch selection's Copy/Paste menu) leaves
focus in the field beneath.

Which layers the pointer can reach is one rule, `Layers::reachable`, shared by
hit testing and by forwarding input to a forwarding catcher (an app's region, a
Wayland window). A forwarding catcher beneath a blocking layer loses any
captured press.

## Dismissing

Closing is requested, not done. `on_dismiss` is called with a `Dismiss` reason
and the owner closes the overlay by writing `open` false, or doesn't (a modal
`DockDialog` pressed on its parent flashes instead). No `on_dismiss` means it
can't be dismissed. Each request goes to one layer:

- Escape nothing on the focus path took: the nearest dismissable layer holding
  the focus, or with nothing focused, the topmost trapping layer.
- Back (Android's gesture, the Back key or mouse button): the topmost layer
  with an enabled `on_back` handler or an `on_dismiss`, stopping at the first
  layer that blocks everything; the root's handlers come last.
  `WindowRequests::handles_back` says whether anything would take it (on
  Android it feeds `setBackHandled`, so otherwise the system's back plays).
- A press outside: every dismissable layer it is outside of, top down, until it
  lands in one or one that blocks everything shuts it out; then up the chain of
  blocking layers that one is nested in, so a press outside a submenu closes
  the whole menu. A blocking layer still lets the pointer reach the blocking
  layers it is nested in, so a menu's rows stay pressable under its submenu. A
  mouse asks on press, a finger on tap, so a scroll starting outside leaves it
  open.
- A press on the `trigger` of a layer above is left to that trigger, so a
  `MenuButton` or `Select` closes on a second click instead of reopening. Every
  component whose button opens an overlay names the button as `trigger` and
  toggles on click. be-host's escaped layers follow the same rule
  (`be_host_runtime::dismissed_by_press`).

Within the layer, the request goes first to the most recently made enabled
`on_back` handler whose component is in the tree (not hidden `ShowKeepAlive`
content), and to `on_dismiss` only when there is none, so Escape, back and a
press outside all play the same slide-away. A layer closed because its parent
closed is told afterwards (`Dismiss::WithParent`). The owner writing `open`
false (`Document::close_overlay`) runs nothing, so a prompt whose dismiss means
"cancel" needs no guard against its own Keep button.

### Back and `BackSlide`

`on_back(enabled, handler)` is a hook like `on_key` that only routes: the
handler hears every phase of the back gesture, or `BackGesture::Invoked` alone
for a key, Escape or press outside. `unstyled::BackSlide` supplies the motion,
as the stacked dock's tabs use it.

On the HTML core back is the browser's: it reaches the same target, found the
same way, as `BackGesture::Invoked` alone (crates/beui-core-html/src/back.rs).
While anything would take it, the page holds one history entry of its own, which
the back button and Android's back pop. A handler that starts disabled never
touches history, and a page reloaded onto its entry takes that entry over.

History is brought in step once on a frame after something changed: a handler or
layer, or a handler's element shown, hidden or removed. When the target goes,
the page takes its entry back with one `history.back()`, and the next popstate,
whatever it was, ends that retraction. A push waits until then, and is skipped
if the page is still on its own entry. Browsers skip an entry made without a
user activation, so a page may get one back per activation before back leaves
it. It is not a `CloseWatcher`, which would also take Esc from the layers' own
Escape handling.

## Focus and layers

Every layer gives focus back when it closes, however it closes. It remembers
the node focused when it opened (or the last focused outside it since) and
refocuses it if it still exists and focus is inside the layer, or nowhere and
the layer traps. A close that moved focus outside deliberately keeps it there.
Components rely on this instead of refocusing their triggers; a forwarding
catcher gets the keyboard back the same way.

What a layer remembers is a focus scope; the document keeps one stack of them
(`focus_scope.rs`). Anything else that takes the focus for a while pushes its
own with `Document::push_focus_scope(holder)` and pops it with
`pop_focus_scope(holder)`, which refocuses only if focus is still inside
`holder` (be-host's region does this when an app asks for the keyboard). A
non-trapping layer (toasts) also gives focus back when its focused part is
removed while it stays open.

Focus requested beneath the topmost trapping layer never lands, so nothing
beneath hears it. It is remembered until the next input event: if the layer
closes first, focus goes there instead of to what the layer remembered, so a
dialog's button can close the dialog and focus something outside in either
order.

## The lock screen

A `BlocksAll` layer at `LayerLevel::Lock` (be-host's `LockScreen`) is above
everything, so anything opened while it is up opens beneath it. It is never
dismissable, focus can't leave it (not even to nothing), `Document::locked` is
true, and listeners above the tree (`on_global_key`, window and host actions)
hear only media keys (`Key::is_media`).

## Modals and dialogs

`unstyled::Modal` is every modal's outer part: an overlay with a scrim that
fades with its presence. `styled::Modal` picks the scrim (`Scrim`);
`styled::Dialog`, `Fullscreen` and `ModalSheet` sit on it.

A dialog's buttons are data
([decision](../../decisions/beui_dialog_actions_are_data_with_roles.md)):
`actions` is a list of `DialogAction`s with an `ActionRole`, and the dialog
places, orders, focuses and binds them (`unstyled::DialogBody`, painted by
`styled::DialogBody`). Cancel, Escape, back, the scrim and the close button
all run the dialog's `on_dismiss`. A confirmation inside a dialog is an alert
of its own opened over it.

`styled::Dialog` is `DialogFrame` (outer part) plus `DialogBody` (title row,
scrolling content, pinned footer). `dialog_presentation` picks, in one place,
between a centred card, a card anchored to a control or rect, a card centred on
the frame, and a `ModalSheet` (a narrow window, below `SHEET_BELOW`, or one a
finger opened). Which anchor to pass:

- A small form about one control: `DialogAnchor::Control` of that control
  (database's Filter button). From a menu row, a `MenuItem` is no node: fill a
  `NodeRef` with `focused_node()` in the row's `on_click` (file-tree's
  Inspect). From a point on a canvas: `Control` of an empty `Frame` in a
  `CanvasItem` there (logic-grid's storage).
- From an action run by the tab's menu, the palette or a shortcut:
  `Rect(use_frame_anchor().get())`, so it points at its tab (canvas Export).
  `use_frame_anchor` is where the window's frame shows to the user;
  be-sdk-beui provides the editor's tab, or `None` when nothing of it shows.
- A large form or embedded editor: `DialogSize::Large`, with the frame anchor
  when it belongs to a frame (be-sdk-shell's Share); host-drawn ones with no
  frame (be-host's Members) pass none and stay centred.
- Alerts and program-level modals pass no anchor.

Code that places its own body uses the parts (`DialogCard`, `DialogBody`, a
`Modal`), as be-host's keep-changes prompt and an app opened as a dialog do.

## Toasts and `Floating`

`unstyled::Toasts` is a `Pass` overlay at an `Inside*` edge with a live region;
`styled::Toasts` takes any anchor, so the desktop stacks toasts in a display's
corner above its bar, at `LayerLevel::Top`. `unstyled::Floating` pins a child
to the top or bottom edge of another node, taking no space (`styled::Tree`'s
reveal button).

## Trees

`unstyled::Tree` gives rows the tree's keyboard and `TreeItem` accessibility
and nothing more; where a pointer selects is the face's choice, through the
handle. `styled::Tree` is the face program code uses. Choosing a row never
expands, collapses or scrolls anything; given `ancestors`, the tree offers a
reveal button and reports `on_reveal` for the caller to expand. The file tree
and the beui inspector are the full examples (row menus, renaming through
`editing`, `TreeApi`).

A row takes the clipboard formats `pastes` names for its key (asked once, when
the row is built) and reports a paste while it has the focus as `on_paste`.
Pastes reach only the focus, so a row menu's Paste calls `TreeApi::focus` on its
row before `request_paste`, as the file tree's does.

Given `reorder`, rows part to show where a dragged row would land. The tree
does not carry the drag: whoever follows it (the tree's own `on_drag_move` and
`on_drop`, or a host's block drag an app only hears as positions) hands each
window point to `TreeApi::drag_over(Some(point), allow)`, which answers a
`TreeDrop` (`Into(row)`, or `Among { parent, after }`, the shape of
be-block-model's `Anchor`) and opens a gap for `Among`; `drag_end(settle)`
closes it, or keeps it until the keys change. The file tree drags rows this way.

## Claiming presses

`capture_presses` on `unstyled::Button` wins a press from a pressable surface
around it (a tab's close cross). An `Interactive` with `claims` takes every
press in an area while given modifiers are held, before `capture_presses` and
outermost first, and the press never reaches a forwarding catcher beneath: that
is how the window manager takes Super+drag from apps and Wayland programs.
`on_press_outside` hears a press outside an `Interactive` at the moment a press
outside asks layers to close (`Document::pressed_outside` does both). An
`Interactive` with `drawing` is a pen canvas a finger can't reach while
`CanvasTouch::Off` ([decision](../../decisions/touch_off_toggle_instead_of_palm_rejection.md)).
An app reports `Document::press_claims` and `drawing_rects` to its host so the
host keeps those presses too.

## Layers that escape their window

A window given a layer space
(`Context::set_layer_space(window, Some(LayerSpace { to_host, bounds }))`) lets
its escaping overlays out of its rectangle. `to_host` maps the window's points
to the host's; `bounds` is where layers may go, in host coordinates. Native
be-host, tests and standalone programs never set one. In the app protocol an
escaped layer is an anchored view and a layer space is its view's anchor space;
be-sdk-beui-platform's `escaped.rs` maps one onto the other
(guides/apps/framework.md, Anchored views).

`Placement::escapes()` decides what escapes: the anchored placements, `Center`
and `Edge` (a sheet on an edge of the bounds, as `ModalSheet` uses) do; `Fill`,
`At` (windows) and the `Inside*` placements (toasts, `Floating`) stay. A
`Center` overlay that escapes is still centred on its window. An overlay that
is a window of its own never escapes. A blocking scrim still covers only its
window.

An escaping overlay is laid out at full size in host units, in a zoomed space
whose coordinates are host coordinates, so a menu from a region shown at half
size is as large as any other and flips against `bounds`. Its rects as
`Rects::get`, hit testing and accessibility see them stay in window
coordinates; `Document::escaped_layer_of(node)` gives the layer and its
`to_host`. Hit testing, focus, trapping, Escape and back are unchanged.

What the host gets:

- `Document::escaped_layers()`: an `EscapedLayer` per layer, in layer order,
  each after its parent (submenus nest). A layer that blocks only its parent
  leaves the owner's view live; the document blocks it itself and never asks
  it to close on a press outside.
- `FrameOutput::escaped`: per layer, that record and a `WindowOutput` painted
  from the layer's top-left in host units, ready for its own surface. The
  layer is left out of its window's painting and rect lists.
- Accessibility: `FrameOutput::window_accessibility_tree` leaves escaped nodes
  out and `escaped_accessibility_tree` describes each layer on its own.
- Input: `Incoming::layer(id, input)` with host positions, which the runner
  maps into the owning window. `Context::dismiss_layer(id)` asks the layer to
  close as a press outside does; it is ignored if that frame's input already
  asked, so a press on a popover's trigger toggles it closed once.

Tests paint a window and its layers together with `Stepped::snapshot_layers`.

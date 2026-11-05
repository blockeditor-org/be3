# Docking and windows

The dock is beui's workspace layout and window manager: panes split from one another, a tab bar on
each, tabs dragged between panes, into groups and out into floating windows, tool bars, dialogs
hung from tabs, and a stacked one-screen mode for phones. The second half of this file is the
multi-window document underneath: how a program shows several top-level windows and how a host
opens them.

The demo's Docking, Dock bars and Window manager pages (`crates/beui-demo`) exercise most of what
follows.

## Crates

- **beui-dock-state**: `DockState`, plain data with no reactivity: surfaces (the main one and one
  per floating window), splits, panes (leaves), tabs, groups, locks, history, the switcher, bars,
  joined groups. `layout_surface` turns a surface into rectangles. Most behaviour is tested here,
  without a document.
- **beui-dock** (the client): the declaration components, `DockingLayout`, `DockDocuments`, the
  per-tab controls (`use_dock_tab()`, `use_dock_bar()`, `dock_menu`), an empty pane's
  `use_dock_pane()`, and the seam to a host
  (`link.rs`).
- **beui-dock-host**: `DockHost`, which lays out and draws a declared dock and owns the drags, the
  keyboard, the window manager (`unstyled/hosting/`), bars, dialogs, fullscreen and the switcher.
  `unstyled::DockHost` paints nothing; `styled::DockHost` draws it.

## Declaring and hosting

A `Docking` only declares: its children describe the starting layout and the full set of tabs. A
`DockHost` above it lays the dock out and draws it where the `Docking` is. An editor's frame already
has one (be-sdk-beui), so an editor writes only the `Docking`; a program, test or shell that needs
the host's settings (`drag_modifier`) or its `DockHostLayout` puts one around its `Docking`.
A `<ListItem>` that places the dock, and `@test_id` and `@node_ref`, go on the `DockHost`.

```rust
let layout = DockingLayout::new();
let host = DockHostLayout::new();
view! {
    <ListItem grow=1.0><DockHost layout=host>
        <Docking layout>
            <DockSplit id="root" direction=Direction::Horizontal fraction=0.25>
                <DockPane id="files"><DockTab id=Key::Files title="Files"><Files /></DockTab></DockPane>
                <DockPane id="editors" empty={move || view! { <Nothing /> }}>
                    <ForEach keys={open}>|key: Key|
                        <DockTab id=key title={title_of(key)} on_close={move || close(key)}>
                            <Panel key />
                        </DockTab>
                    </ForEach>
                </DockPane>
            </DockSplit>
        </Docking>
    </DockHost></ListItem>
}
```

Two handles: `DockingLayout<K>` is the client's, keyed by the caller's `DockKey` (where everything
is, `show`, `focused`, `mode()`, `snapshot`/`restore`, bars by key, fullscreen);
`DockHostLayout` is the host's, keyed by `TabId` (`DockingLayout::tab_of` converts): its area and
covers, its windows and the window manager, the switch.

**Reconciling.** The declared tabs are the whole set: a tab the code stops declaring is closed, one
it starts declaring is placed (beside the tab declared before it, else the nearest open declared
tab, else a declared window, else a window of its own). Closing is a request: `on_close` gives the
tab a close button, and the tab goes only when the caller stops declaring it. Two traps follow:

- A tab whose declaration disappears for a moment (a `Show` that flips, a `ForEach` rebuilt from new
  keys) is closed and loses its place, history and panel. Keep each tab's declaration in one stable
  place.
- Restore a layout before the effects of the write that declares its tabs run (writes made together
  with no `flush()` between qualify); a tab not declared when the dock reconciles is closed.

Every container's `id` is saved with the layout, so a saved layout finds its panes after the code
changes.

**Saving.** `DockingSnapshot` has no serialized form of its own. To save one, use be-sdk-beui's
`SavedDockLayout::of(snapshot)`, which is the app protocol's `DockLayout`. Saved: the tree, the
order tabs were shown, history, homes and unlocked places, bars and compact sizes. Not saved: a
switch in progress, fullscreen, window snaps, hidden windows and dock dialogs.

**Documents.** A tab's key names the tab, not what it shows. Lists that open what they name into
tabs (the demo's catalog, be-launcher's pull requests) use `DockDocuments` with `DockDocumentTabs`
and `DockDocumentBody`, which reuse the last-shown slot (unless Ctrl/Cmd asks for a new one) and
rebuild a body only when its document changes.

## Panels, focus and keys

A tab's panel is built the first time the tab is shown and belongs to the dock, not the pane: the
pane shows it through a `Portal`, so the panel keeps its nodes, scroll, caret and state when hidden,
dragged, floated, or switched between tiled and stacked. A panel no pane shows is not laid out and
not read by a screen reader.

Pressing a pane's chrome (bar, tab, grip) gives the keyboard to the tab it shows: the bar is an
`Interactive` with `press_focus=false` (guides/beui_keyboard.md) and
`beui-dock-host/src/unstyled/keyboard.rs` puts the focus back where that tab last had it, or on its
first focusable. Content whose keyboard lives in another document (a program's window, a child
block) follows `DockTabControl::keyboard` and `brought_forward` and asks its host for the keys
(`focus_child`, guides/apps/children.md, Focus). The other way, when such content takes the focus
there it shows its tab (`DockTabControl::show`) or, in an empty pane's view, focuses that pane
(`DockPaneControl::focus`), which brings the window in front. A focused empty pane in a joined
group has no tab, so `DockState::focused_or_host` stands the group's host tab in for it: that is
what `DockingLayout::focused` and `DockHosted::focused` report.

Ctrl+Tab walks the focused pane's tabs (an `on_shortcut`, so it works from inside a text input).
The dock binds no other key: the switcher and window actions are actions the shell binds.

## Modes

The dock picks its mode from its own size (`DockingLayout::mode()`; the workspace reads it to know
it is on a phone; thresholds are the styled `Docking`'s `NARROW_WIDTH`, `SHORT_HEIGHT` and
`PHONE_LANDSCAPE_WIDTH`):

- **Stacked** (`DockMode::Stacked`): the focused tab fills the dock under the dock's own stacked bar
  (icon and title, a count that opens a sheet of open tabs, back). No tab bars, splitters or
  windows, but the whole state is kept, so widening tiles it again. Locks are ignored while
  stacked.
- **Gathered** (`DockMode::Compact`): a compact tree with a bare stage pane (an editor's stage,
  `DockState::stage_of`) keeps the stage and gathers every other pane's tabs into one foldable pane
  below or beside it. Its sizes are saved apart from the tiled ones (`DockCompact`), so switching
  sizes mixes neither.

## Menus and pinned actions

A tab hands the dock its menu with `dock_menu(items)`, shown behind More; actions marked `.pin(..)`
go into the bar. A menu is data, not a node, so a dock showing another app's tabs draws it from rows
it was sent (`ActionBuilder::detached`) and passes the pick back. `unstyled::ActionBar` does the
split between bar and More for any toolbar.

## History and the switcher

Every change of the focused tab is a `DockVisit` in `DockState::history`. Back undoes the last
visit, closing a tab the visit opened unless the tab says `back_closes=false` (a program's window,
whose close ends the program). The switcher walks tabs in the order they were shown, across panes
and windows, like Alt+Tab; `dock.switch.next`/`previous`/`cancel` are actions and the shell picks
the keys. Escape always cancels a switch, wherever the focus is: the overlay holds no focus, so the
dock listens at the window.

## Windows and the window manager

Each surface lays its tree out over its rectangle; panes and splitters are canvas items at computed
rectangles, not nested boxes. A floating dock window is a beui `Window` (below) with resize grips in
a ring outside it, so content gets every press up to its edge.

- Where nothing opens windows, the dock moves and resizes the window's overlay itself.
- Where a host opens windows (`opens_windows`: space's panels, be-host's desktop), each dock window
  is a real window: its bar and grips call `begin_move`/`begin_resize` and the host moves it. The
  dock keeps each `Placed` report's `HostPlacement` and hands it back when the window reopens. A tab
  dropped where no window is (`on_drop_outside`) floats into a new window at the pointer.

`DockState::windows` is bottom to top, the focused window is on top, and `Document::layers()`
follows. Drop targets are resolved in the window the `DragPoint` names, in that window's
coordinates, and never in a window a non-dock layer covers at the pointer.

The window manager (`beui-dock-host/src/unstyled/hosting/manager.rs`, `snap.rs`,
`beui-dock-state/src/hidden.rs`) is on `DockHostLayout` for any shell: areas (`set_areas`, one `DockArea`
per display: the whole screen, where a window goes fullscreen, and the work area it is kept, snapped
and maximized in; a window belongs to the area it overlaps most), snapping, hiding and show-desktop,
cycling a `DockTab`'s `group`, and `window_actions(disabled)`, which returns `dock.window.*` and
`dock.switch.*` actions with no keys, acting on the focused window. The desktop binds them
(guides/desktop.md); the demo's Window manager page puts them in a menu.

`DockHost`'s `drag_modifier` makes the dock a window manager while those modifiers are held: a
primary drag anywhere in a floating window moves it, a secondary drag resizes it (or moves the
nearest split on a docked pane). Each window and pane claims those presses, so content never sees
them (`unstyled/modifier_drag.rs`; the desktop sets Super).

## Dragging, groups and locks

Dropping a tab on a tab bar inserts it where the bar parts for it (each bar is a `Reorder`), on a
pane's middle groups it with the tab the pane shows (`DockDrop::Group`; the highlight names that tab
unless tidying would only fold it into the pane), on an edge splits it; Alt while dropping floats it. Dragging a pane's grip carries the pane.
A group is a tab holding a dock tree of its own (`Entry::Group`); the tree tidies itself after every
change (a one-tab group becomes the tab). A pinned group is never tidied away and admits only its
own tabs.

A `locked` pane makes an area, and every tab declared in it lives there (`DockState::home`). While
locked, a tab at home moves and splits freely inside the area but cannot leave or float; tabs from
elsewhere are kept out. Locks belong to places, not tabs. Over a drop the state refuses
(`DockState::admits`, `admits_leaf`) the dock shows the refused marker (`dock.refused`) instead of a
drop marker, and refuses the drop on the board (`Take::Refuse`), so a release there does nothing and
the tab glides back to its bar; check `admits` first when a drop test does nothing.

## Nested docks and the host seam

Every `Docking` declares through a `DockHostLink` (`beui-dock/src/link.rs`), plain data both ways:
`DockDeclaration` up, `DockHosted` and `DockHostEvent` down, `DockRequest` up, `DockBodies` for the
host to place. The link comes from the `DockJoin` of the tab body the `Docking` is built in, else the
`DockHostSlot` context above; with neither it panics. A `DockHost` provides a slot that draws the
dock in place; a platform's slot (`DockHostSlot::new`) hosts it elsewhere, which is how a dock in an
app reaches the host across the app protocol (be-sdk-api PROTOCOL.md, "Docks cross apps").

A `Docking` inside a `DockTab` of another joins it and draws nothing. Nesting happens in the
client: the outer dock splices the inner one in as a pinned group keyed `joined:<tab>`, so a host
only ever sees one declaration, and the inner `DockingLayout` goes through the outer dock. Each layer saves its own layout: the outer snapshot keeps only the hosting tab, so the
inner dock's owner saves the inner snapshot (be-sdk-beui does, through a `DockKeeper` in every
editor's frame; an owner that saves its dock itself provides `DockKeeper::none()`, as be-sdk-shell's
`Workspace::new` does). A `DockHost` hides any `DockJoin` around it, so a dock under it stands
alone even inside another dock's tab.

## Dialogs, tool bars, fullscreen

- **`DockDialog`**: a dialog hung from a tab (or another dialog), shown exactly while its parent tab
  is, never part of the `DockState` and never saved. `modal` blocks only the parent's panel. Use it
  for content the dock must hold (a program's dialog, be-sdk-shell's `WindowDialog`); a form opened
  from a control inside a tab body is the shared `Dialog` (guides/beui/overlays.md), which finds
  its tab through `use_frame_anchor()`.
- **`DockBar`**: a tool bar belonging to a tab (or, with no `parent`, to the whole dock), drawn
  wholly by its owner, grip included (`DockBarGrip`). It shows only while its tab is shown, over an
  edge, docked in a band, or popped into a window. At the root, bars are placed by key
  (`{block type}/{key}` across apps), so every canvas's `tools` bar shares one placement and one
  popped-out window. The dock offers no way to show a hidden bar again: its owner must.
  Dragged by its grip (`unstyled/bars.rs`), a bar docks in an edge's band only within half a
  band of that edge (the band there, else the bar's thickness), and holds on to the band to half
  a band past it; a bar grabbed floating over an edge must first be pulled halfway from where it
  was grabbed towards that edge. Let go at speed outside every band, it is thrown to float over
  the edge its path runs into; inside a band it docks there.
- **Fullscreen**: one tab at a time (`DockState::set_fullscreen`) moves its panel into an overlay,
  or a hosted `Window`, over an area; leaving puts it back without rebuilding. The dock binds no key
  to leave: whoever enters offers the way out.

## Test ids

A pane's bar is named after the pane's `id` (`dock.grip.<id>`, `dock.<id>.menu`,
`dock.<id>.pin.<action>`); a pane the user made, and the stacked bar, use the bare prefix `dock`. A
tab keyed by `TabId` or `u64` carries it (`dock.tab.<tab>.close`). `DockHost`'s `api`
(`DockApi::state`) hands a test the state.

## Windows and window hosts

A program has one `Document`, one reactive graph and one focus, but may show several top-level
windows. The code is `beui-core/src/window.rs` and `window/`; the component is `beui::Window`
(`beui-view/src/window.rs`).

- **Output**: `FrameOutput::windows` holds a `WindowOutput` per window (`WindowId::MAIN` first)
  with its own layers, damage and pixels per point; what happens once per program stays on
  `FrameOutput`.
- **Requests and reports**: the program asks with `WindowRequest`s (`Open` with a `WindowRole`,
  `Resize`, `Raise`, `BeginMove`, `Place`, `Close`), and the host answers with `WindowReport`s
  (`Resized`, `Placed`, `Moved`, `Rastered`, `Focused`, `Closed`) applied before the frame lays
  out. A `WindowPlacement::Host` is opaque to the program: store it from `Placed` and hand it back.
- **Hosting** (`Context::window_hosting`): `None`, every `Window` is an overlay in the main window;
  `Any`, a layer opens every window (space; `Context::opens_windows` is true only here); `Placed`,
  the platform opens only windows opened with a placement it handed out (an app's views), the rest
  are overlays inside their window. Hosting may change while running, and a `Window` follows. A
  platform with no main window of its own calls `Context::set_shows_main(false)`.

A `Window` is an `Overlay` with a window spec, so a program looks the same hosted or not.
`WindowHandle::begin_move`, `begin_resize` and `close` return false when the window is an overlay,
and then the caller moves it itself.

Across windows:

- Each `Layer` names its window, and hit testing, claims and forwarding use
  `Document::window_layers(id)`, so a press never reaches another window's nodes at the same
  coordinates.
- Focus is one across windows. A host's `Focused { focused: false }` takes the focus out of that
  window and `true` gives it back; with nothing to give back, a dock window hands it to its shown
  tab (`window_focused` in `beui-dock-host/src/unstyled.rs`), else the document focuses the first
  focusable. Under `Placed` hosting, `Focused` moves the focus between windows instead.
- Key listeners at `KeyAt::Window` and actions belong to the hosted window they are registered in
  (`Document::host_window_of`), so two app views each keep their own Escape and shortcuts.
- Damage, scroll copies and raster scale are per window; every window is laid out at the
  context's scale (`Context::window_painter`).
- Pointer input carries its window. A press held across into another window keeps dragging, its
  `PointerPress` naming the window under the pointer. Forwarding catchers (app regions, Wayland
  windows) get it in their own coordinates once the host has reported both windows' `Moved`.
- Dragging out of every window: a host sends `PointerLeft` while a button is held over no window;
  the drag board then drops nothing and calls `Draggable`'s `on_drop_outside`. This happens only
  when the host opens every window; elsewhere a drag out drops at the last point inside.
- `be_lib_ui_types::Input` has no window: it is also the app protocol's input, and an app region is
  always one window.

**Testing.** `testing::Stepped::hosting_windows` (be-sdk-test's `DocumentTest::hosting_windows`)
runs a fake host that opens every window at its size and reports back the next frame: `frame_in`,
`take_window_requests`, `report_window`, `windows()` and `snapshot_window(name, id)`.
`Stepped::hosted` runs under a real window-hosting layer such as `beui_layer_space::Space`, as
be-host-wayland's space test does. Dock window-manager tests live in
`beui-dock-host/src/unstyled/tests/`.

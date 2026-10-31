# Anchored layers, dock bars, and a camera that follows its region

One pull request, built by parallel agents (guides/parallel_agents.md). Done: the design below is what was built, with the differences listed at the end; the protocol, guides/adding_a_plugin_editor.md and guides/beui.md describe it as it is.

## Goals

1. Anything that points back to where it came from (context menus and their submenus, popovers, tooltips, the text-selection toolbar, small dialogs opened from a control) and any sheet attached to an edge escapes the plugin frame that opened it, with zero frames of delay on native.
2. One mechanism for all of it: an *anchored layer*. It replaces Popup and Overlay window roles, `overlays_in_windows`, dock card dialogs with an anchor, `ShellCommand::Rename` and `ShellCommand::AppMenu`.
3. The infinite canvas's tool dock becomes a dock *bar* the user can attach to any edge of the canvas's pane (inside or outside the body) or pop out; each dock layer saves its own layout, the canvas's in per-device settings.
4. A region move never changes a pan-and-zoom camera; a resize keeps the centre of the content area once the camera was moved, and keeps it fitted before that, with no frame of lag.

## Design

### Camera follows its region (workstream A)

- `BlockFrameView` (`crates/block-app/src/compositor/editors.rs`) no longer adds the opposite of a region's move to `pan` (the `state.center` correction). The content rect is centred on the content area's centre plus `pan`, so a moved region carries its world and a resize keeps the centre. Before the camera is `moved`, `keep-fitted` keeps fitting as today.
- The frame report gives the content area as insets from the region's edges instead of a rectangle, so the host computes the new content area during the resize itself and sends the new view with the screen request in the same frame. Only a frame where the plugin changes its own insets (a sidebar moving into a sheet below `NARROW_WIDTH`) lags one frame.
- No editor declares an anchor: every pan-and-zoom editor keeps the centre.

### Anchored layers (workstream B)

A layer is painted output of the region that opened it, allowed outside the region's rectangle. The host draws it from the frame commit, like today's `floating` rects, so nothing is mounted and there is no round trip.

- **Layer space.** Each screen request carries `to_host` (region coordinates to the app window's logical coordinates, at the view the frame is laid out against) and `bounds` (where layers may go, in host coordinates): the app window for a trusted plugin, the plugin's own box for a confined one. A child's bounds never exceed its parent's.
- **Layers in the commit.** `frame-commit.layers`: id; parent (the region or another layer, for submenus); placement (anchored to a rect in the parent's coordinates, or attached to an edge of the bounds for sheets); the rect the plugin chose in host coordinates; surface; painted and floating rects; scrims; flags `dismiss-on-outside-press`, `keyboard`, `blocks-owner`; child placements (so a layer can embed another editor).
- **Plugin places and draws.** The plugin lays the layer out against `bounds` (flip, slide, resize as beui's `Floating` already does), allocates its surface itself from a pool (natively the same synchronous `configure_surface` it already uses), paints it and commits it with that frame. The host only clips to `bounds`.
- **Host composites and routes.** Layers are drawn after every region, in the order they opened, clipped to `bounds`. A region move moves its layers rigidly (`to_host` now versus the one laid out against); a camera pan does nothing to layers (the plugin repositions or closes them on its next layout). Layers are hit-tested first; input reaches the owner tagged with the layer id. A press outside a dismissible chain closes it (the owner hears `layer-dismissed`, which beui maps to `Dismiss::OutsidePress`) and still reaches what it landed on. `keyboard` gives the owner the keyboard as `wants_keyboard` does. `blocks-owner` keeps presses on the owning frame from its content while the layer is open.
- **beui.** A window gets an optional layer space. With one, anchored overlays (menus, `ContextMenu` and submenus, popovers, tooltips, the selection toolbar, `Floating`) and edge-attached sheets are laid out against `bounds` in host space and painted as their own layers. Modals centred in their region, toasts and banners stay in the region. Without a layer space (native block-app, tests, standalone) nothing changes.
- **Accessibility.** A layer's tree goes in its owner's `describe`, in host coordinates, so screen readers and `drive` find it where it is drawn.
- **Deleted:** `overlays_in_windows`, the Overlay and Popup window roles and `desktop::dismiss` for popups, `dock-dialog.card`/`anchor` (dock dialogs keep only windows and modals), `ShellCommand::Rename`, `ShellCommand::AppMenu`.

### Embedded popups (workstream C)

A layer can hold a child editor; its content appears one frame after the layer, which is fine.

- **Picker.** `Editor::pick_block(filter, anchor, cb)` keeps its API. The asking editor opens an anchored layer, draws the card and beak, and places `SubregionContent::Picker` inside it (drawn by workspace-ui's picker view). The layer takes the picker's last reported size (256x320 the first time). On touch it is a sheet attached to the bottom edge of the bounds.
- **Rename.** The asking editor opens a layer holding a new `SubregionContent::Rename(block)`, drawn by workspace-ui's rename view.
- **App menu.** A layer holding `SubregionContent::Host(AppMenu)`, drawn by block-app, placed by the asking editor.

### Dialogs (workstream H)

One `Dialog`, presented by kind and size, with an optional anchor:

1. Small per-object forms opened from a control (text settings, logic-grid storage, calendar event form, file-tree Inspect, database filter, canvas Export, Rename): a card with a beak on desktop, a bottom sheet on phones. They escape the frame, have no scrim, close on an outside press, set `blocks-owner`, trap focus and give it back.
2. Confirmations (Alerts): centred over the frame that asked, with a scrim, inside its region. Never a sheet.
3. Large forms and embedded editors (block creation, artifact settings, Share, Members, Contacts, Agents, conflict compare): attached to the owning frame, not a point; a full-height sheet on phones (escaping, attached to an edge). The anchor only drives the opening transition.
4. App-level modals with no origin (lock, reauth, onboarding, add device, sync errors): host-drawn, centred over the app. Unchanged.
5. Program dialogs (Wayland/X): windows centred on their parent. Unchanged.
6. Command palette: top-centred overlay. Unchanged.

Anchor fallback: the control that opened it; if that was a menu row that has closed, the thing the menu was opened on (a context menu's target, a dropdown's button); if the command came from a menu another layer drew (the frame menu on the tab bar) or from a shortcut or the palette, the editor's tab, whose rectangle in host coordinates the root dock fills in on menu picks (`dock-update.pick`, the child-menu pick) and on request; for a child with no tab, the rect its parent placed it at; if none of those is visible (a fullscreen tab), a zero-height rect at the top centre of the owning frame. A small dialog is always one placement, anchored to a rect; it draws its beak only when the anchor is something the user can see (control, row, tab), not the synthesized top-centre one, so `HangFrom` and `DialogAnchor::Frame` go. Large dialogs stay centred on their frame and app-level modals over the window. On phones, sheets. `DockDialog`'s presentation policy moves into this shared `Dialog` (workstream I, after B4 and D2).

### Web (workstream G)

Each plugin worker gets one overlay canvas covering the app viewport, above the app canvas with pointer events off, created and transferred on its first layer and kept. The worker draws its layers into it. The page sets a `clip-path` to the plugin's bounds and raises the canvas of the plugin that opened the newest layer.

### Dock bars (workstream D)

- **Bar entity** in beui-dock: a tab-like entry whose body its owner draws, with its own grab handle (it calls `begin_move()`, as a dock window's bar does; the root carries the drag and shows drop targets). Placements: an edge of its pane inside the body (floating over it) or outside it (a band beside the body, inside the pane), or popped out into a dock window. The owner lays the bar out horizontally or vertically from the edge it is told.
- **Bare pane:** a pane of one tab with no tab bar, so the canvas's joined group shows no second tab strip.
- **Across plugins:** a bar kind of `dock-item`, its body a region of the owner's like `tab(t)`.
- **Canvas:** builds a `Docking` holding a bare stage tab and its tool bar; `ToolDock` becomes the bar's body; `BottomDock` goes. Popovers from the bar are anchored layers. The selection tools sit against the same edge. Standalone or confined, the `Docking` draws itself inside the frame.
- **Shared popped-out window:** one dock window per bar key, showing the bar region of the most recently focused declarer; focus on it does not count in the dock's history; each declarer keeps its bar region built; the window takes the shown bar's size, pinned at the handle's corner; it hides while no declarer is shown.

### Dock layouts saved layer by layer (workstream E)

- A layer's snapshot includes where its moved-out entries are (a bar's edge or popped-out rect relative to the owner's frame); the root's snapshot stops saving joined subtrees and keeps only the hosting tab.
- block-editor-beui saves the layout of every editor whose dock joins another plugin's in `DisplaySettings.docks` through `LayeredSettings` (this device), keyed by block type, restores it once the settings are ready and the dock has declared, writes on change, and every open instance follows (no per-editor code). The workspace keeps saving its own layout in its `EditorView`.

### Views (workstream V)

Today an editor shows things through region kinds that each carry their own features: `frame` (camera, frame menu, bar actions, taking over the frame, popping out), `tab(t)` and bars (placed by the root, with `dock-tab.menu`), `window(w)` (opened by `window-request` after a round trip), `display(n)`, `preview`, `artifact-settings`, and layers (in the frame report). V replaces them with one concept.

- **A view** is a surface an editor instance draws, with an id unique in the instance. View 0 is the instance's main view (today's frame). Frame, tab, bar body, window, display, preview, artifact settings and layer stop being region kinds.
- **Features belong to the view, not to its kind:** its menu (merging the frame menu and `dock-tab.menu`), bar actions, child placements, the camera (on the view that holds `editor.content`), the layers hanging from it, chrome shown, keyboard requests, intercepted keys, accessibility.
- **Every view has exactly one placer**, and placement is separate from the view:
  - *parent*: a child placement names the child's view (its main view, its preview view);
  - *dock*: the root dock places a view as a tab body or bar body (dock tabs and bars name view ids);
  - *host*: a window (in-app panel or OS window) or a display;
  - *anchored*: the view places itself against an anchor in its parent view (today's layers).
- **Docking is only a question of who places the views.** A non-docked editor's main view is placed by its parent. A docked editor declares its views as tabs and bars and the root places them. The canvas joining the workspace moves its stage view and bar view to the root's placement; no feature is lost because features follow the view.
- **Popping out** changes a view's placer to a host window, the same move for a tab, a bar or a whole editor.
- **Editors create views themselves.** An editor creates any view it needs in its commit (an anchored view, a window view with the rect it wants, a tab or bar view it declares to its dock); the host validates, clips to the layer space's bounds and places it in that frame, so opening a window costs no frame either. `window-request.open` merges into this path. Views another placer asks for (a parent asking for the preview view, the host asking for a display) are requested by that placer, and the editor answers with the view in the same commit path.
- **One way for an editor to create a view:** every view an editor creates (anchored views, windows, later popped-out views) is a screen the plugin numbers itself (`ScreenId::opened`), laid out, painted and committed in the frame that creates it; its placement decides whether it is anchored or a window. Anchored views stop being surfaces listed in the frame report and gain everything screens have (child placements, their own input and accessibility).
- **Popping out** a dock tab keeps the root as its placer: the root holds the only dock state, so it draws the dock window's chrome (tab strip, grab bar, drop targets, menus, locks) in a window view it creates with no frame of delay, and places the popped-out tab's body in it.
- **The host still sizes views placed by others:** a view placed by a parent, the dock or a display gets its size, scale and layer space from the host (today's screen request, per view); a view the editor places itself (anchored, a window it opens) carries the rect it chose.

- **Whether chrome joins depends only on where an editor is placed:** placed in a dock tab, its dock joins and the root draws its chrome; placed anywhere else (a card, a confined plugin), its dock draws itself inside its own view; a child taking over its parent's frame keeps its stage a frame child drawn over the parent's content (so the parent stays visible and pressable), while its bars join the parent's pane in place of the parent's bars, which hide until the child lets the frame go: while the child is focused you see the parent and the child's chrome, never the parent's chrome (the presentation is the exception: its own chrome stays, and the slide's editor joins through the stage tab).
- **Presentation as a docked editor:** the stage is a bare tab in a locked area whose body places the slide's child in that tab (so the slide's editor joins through it), the filmstrip a tab in its own locked area, the toolbar a bar on the stage's pane. Presenting, a card or a preview declares no dock, only the stage.
- **Several bars on one edge** stack in a fixed order, the outer editor's nearer the edge; dropping a bar on an occupied edge slots it in.
- **Docks that never join keep their layout too,** saved by the editor's own plugin (block-editor-beui, per device, keyed by block type) exactly as joined ones are; the root never saves another plugin's layout.

V runs after B4 and before C, F, I and D4, which build on views. D3's phase 2 (generalizing the frame to the tab that holds the content) is part of V.

## What was built

Every workstream landed (A, B0-B4, C, D1-D4, E1-E2, F, G, H, I, K, P, R, T, V0-V4, CL1-CL3, BG and the final pass). The design above is what the code does, with these differences:

- **Names.** "Layer" became the *anchored view* and "layer space" the *anchor space* (`screen-request.anchor-space`): an anchored view is a screen the plugin numbers itself (`ScreenId::opened`) and reports like any other, so `frame-commit.layers`, `layer-id`, `child-placement.in-layer` and `timed-input.layer` went with V4. beui keeps its own words: an overlay that escapes its window is an escaped layer, its window's space a layer space; beui-platform-plugin (`escaped.rs`) maps each escaped layer to one anchored view. The host and compositor say view too (`PlacedView`, `PluginView`, `HostAnchored`, `AnchoredViews`), and block-editor-plugin's `Regions`/`Region` are `ViewHandler`/`ShownView`.
- **Sheets.** On touch or a phone every embedded popup (picker, rename, app menu) is a bare view the asker attaches to the bottom edge; the host tells a child placed in a view attached to an edge that it is a sheet (`open-picker.sheet`, `open-rename.sheet`, `PanelReport::sheet`) and the child draws the sheet.
- **Popping out** a dock tab keeps the root as its placer (V4): the root draws the dock window's chrome in a window view it creates with no frame of delay and places the tab's body in it. No "change of placer" request exists.
- **Stacked or tiled** is still decided from the dock's measured width, inside the layout loop (one frame late on native, within the frame on HTML); panes and splitters are arranged in the same layout (K1).
- **The presentation** keeps its own chrome when a slide's editor takes its frame over; the slide's editor joins through the stage tab.
- **Dialogs** have no `HangFrom` or `DialogAnchor::Frame`: a small dialog points at its control, the tab (`use_frame_anchor`) or a zero-height rect at the top centre (FI).

Left: a joined bar's drop across layers only reorders within its own layer (K2); a child placed Above the pixels of an anchored view shows under them on the web (G); a popup child resized before it draws again is shown stretched to its new rect for a frame.

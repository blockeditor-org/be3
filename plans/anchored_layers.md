# Anchored layers, dock bars, and a camera that follows its region

One pull request, built by parallel agents (guides/parallel_agents.md). This file is the design and the plan; agents read it before starting and the coordinator keeps it current.

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

1. Small per-object forms opened from a control (text settings, pixel-art Resize, logic-grid storage, calendar event form, file-tree Inspect, database filter, canvas Export, Rename): a card with a beak on desktop, a bottom sheet on phones. They escape the frame, have no scrim, close on an outside press, set `blocks-owner`, trap focus and give it back.
2. Confirmations (Alerts): centred over the frame that asked, with a scrim, inside its region. Never a sheet.
3. Large forms and embedded editors (block creation, artifact settings, Share, Members, Contacts, Agents, conflict compare): attached to the owning frame, not a point; a full-height sheet on phones (escaping, attached to an edge). The anchor only drives the opening transition.
4. App-level modals with no origin (lock, reauth, onboarding, add device, sync errors): host-drawn, centred over the app. Unchanged.
5. Program dialogs (Wayland/X): windows centred on their parent. Unchanged.
6. Command palette: top-centred overlay. Unchanged.

Anchor fallback: the control that opened it; if that was a menu row that has closed, the menu's trigger; if a shortcut or the palette opened it, hang from the top of the owning frame; if nothing owns it, centre over the window. `DockDialog`'s presentation policy moves into this shared `Dialog`.

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
- The canvas saves its layer's snapshot in `LayeredSettings` (this device), restores it behind `Loading` before declaring, writes on change, and every open canvas restores when it changes. The workspace keeps saving its own layout in its `EditorView`.

## Workstreams and waves

| Wave | Work |
|---|---|
| 0 | B0 protocol types and PROTOCOL.md (no behaviour) · A1 camera · D1 beui-dock bars in-process with a beui-demo page |
| 1 | B1 host (block-app compositor, routing, dismissal) · B2 plugin runtime (layer surfaces, commit, input) · B3 beui (layer space, escaping overlays, `Dialog` anchor and presentation) · D1 then E1 |
| 2 | B4 integration, app tests, accessibility, `drive`, delete Overlay windows and `overlays_in_windows` · D2 bars across plugins |
| 3 | C picker, rename, app menu · F dock dialogs as owner-drawn layers · G web · D3 canvas toolbar · E2 settings · H dialog call sites |
| 4 | D4 shared window · cleanup of dead mechanisms · guides (PROTOCOL.md, adding_a_plugin_editor.md, beui.md, pan_and_zoom.md, desktop guides) |

File ownership, to keep parallel agents apart: B1 `crates/block-app/src/compositor/*`, `plugin_host/runtime.rs`, `plugin_host/instances.rs`; B2 `crates/block-editor-plugin`, `crates/block-gpu-guest`, `crates/beui-platform-plugin`; B3 `crates/beui-core`, `crates/beui-view`, `crates/beui-types`, `crates/beui-components-*`; D1 and E1 `crates/beui-dock`; G `crates/beui-platform-web`, `crates/block-app/src/plugin_host/web`.

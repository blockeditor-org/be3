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
let layout = DockingLayout::<Key>::new();
let host = DockHostLayout::new();
view! {
    <ListItem grow=1.0><DockHost layout=host>
        <Docking layout>
            <DockSplit id="root" size={SplitSize::Fraction(0.25)}>
                <DockPane id="files"><DockTab id=Key::Files title="Files"><Files /></DockTab></DockPane>
                <DockPane id="editors" empty={move || view! { <Nothing /> }}>
                    <ForEach value={open}>|key: Key|
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
covers, its windows and the window manager, the switch. `layout` is optional: a `Docking` given none
makes its own. The declarations take no key type: a tab's `id` is a `DockId`, which any `DockKey`
passes as. A dock read through a `DockingLayout<K>` keys every tab it gives an id with a `K`
(debug builds assert it); a tab left without one has no key there. Tabs, dialogs and bars share
one id space: declaring two with the same `id` in one dock is a bug, and the dock asserts against
it in every build.

**Defaults.** Whatever a declaration leaves out, the dock derives, so a layout says only what
differs (an editor's stage and side panel are about ten lines, guides/apps/editor.md):

- A split's and a pane's `id` is its place under its parent (`0`, `0.1`), a stage's `stage`, and a
  tab's its pane's key and its title (`0.1/Inspector`). A node keeps the key it derived while it
  is declared, and one that takes a key some node already holds steps past it (`0.1#1`), so a
  node a `Show` brings in front of another derives a key of its own, which depends on what was
  declared first. Give an `id` to what a saved layout, a test or the code names. A tab with a
  derived id has no key in a `DockingLayout<K>`, and its typed snapshot leaves it out.
- `stage` marks an editor's stage (`DockPane`): bare, its one tab untitled (a joined dock is named
  after the tab it joined), focused when the dock starts unless `Docking`'s `focus` names a tab, and
  what the gathered mode keeps. A dock with a stage is an editor's layout: every pane is locked,
  and the body of each tab outside a bare pane is the standard panel (a `Surface` fill, a `Scroll`
  filling it, padding inside, `DockPanel` in `styled.rs`) unless the tab is `bare`.
- `chrome` marks a pane declared only while the frame shows its chrome (`provide_dock_chrome`,
  which be-sdk-beui provides from `Editor::chrome_shown`); a split left with one child is that child.
- A split with no `size` gives `SIDE_PANEL_WIDTH` to the side without the stage, and halves when it
  has none.

**Reconciling.** The declared tabs are the whole set: a tab the code stops declaring is closed, one
it starts declaring is placed (beside the tab declared before it, else the nearest open declared
tab, else a declared window, else a window of its own). Closing is a request: `on_close` gives the
tab a close button (the tab's only pointing-hand cursor), a pane or window whose every tab has one
gets a close button on its bar too, even with a single tab, and the tab goes only when the caller stops declaring it. Two traps follow:

- A tab whose declaration disappears for a moment (a `Show` that flips, a `ForEach` rebuilt from new
  keys) is closed and loses its place, history and panel. Keep each tab's declaration in one stable
  place.
- Restore a layout before the effects of the write that declares its tabs run (writes made together
  with no `flush()` between qualify); a tab not declared when the dock reconciles is closed.

Every container's id is saved with the layout, so a saved layout finds its panes after the code
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
dock holds one `KeepAlive` and each pane shows a panel through a `Kept` keyed by its tab (bars'
panels likewise), so the panel keeps its nodes, scroll, caret and state when hidden, dragged,
floated, or switched between tiled and stacked. A panel no view shows is paused, not laid out and
not read by a screen reader; it is dropped once its tab leaves the dock. The panel of a tab whose
own dock joined this one is in `KeepAlive`'s `awake`, so it keeps running, and so is every bar's,
whose content may answer keys while its band is hidden. A panel's `kept_shown()` is the dock's
own, not its `Kept`'s: a pane showing another tab, or building its view again, does not make a
dock joined from it let go. A view of a tab's content that takes the
keyboard while no pane or fullscreen view shows its panel (an app putting it back on a popup's
trigger as a press on another tab closes the popup) does not show the tab again; the back preview,
which only pictures the page a back gesture goes to, does not count. Tab content is built inside the dock's
`KeepAlive`, so a `Kept` in it with no `KeepAlive` of its own joins the dock's, which keeps only
the dock's keys: give it its own `KeepAlive`.

Pressing a pane's chrome (bar, tab, grip) gives the keyboard to the tab it shows: the bar is an
`Interactive` with `press_focus=false` (guides/beui_keyboard.md) and
`beui-dock-host/src/unstyled/keyboard.rs` puts the focus back where that tab last had it, or on its
first focusable. Content whose keyboard lives in another document (a program's window, a child
block) follows `DockTabControl::keyboard` and `brought_forward` and asks its host for the
keyboard in its placement (`keyboard-ask`, guides/apps/children.md, Focus). The other way, when such content takes the focus
there it shows its tab (`DockTabControl::content_focused`) or, in an empty pane's view, focuses
that pane (`DockPaneControl::focus`), which brings the window in front. `content_focused` also
records that the tab's content has the keyboard, until this document or an empty pane takes it,
so a dock that stops showing the tab hands the keyboard to the tab shown in its place, as it does
for focus inside its own panels. A pane switching between joined editors' groups builds the shown
group's body anew, which hands on nothing, so the switch hands it on itself: Next and Previous tab
from the entry that held the keyboard to the tab the newly shown entry shows (`State::cycle`), and
a press on a group's tab to the tab that group shows (`chrome_pressed`). A focused empty pane in a joined
group has no tab, so `DockState::focused_or_host` stands the group's host tab in for it: that is
what `DockingLayout::focused` and `DockHosted::focused` report.

The dock's tab commands are actions (`tab_actions` in `beui-dock-host/src/unstyled.rs`), so a
command palette lists them: Next tab and Previous tab walk the focused pane's tabs (for a joined
editor's stage, the tabs of the pane its group stands in, `DockState::standing_leaf`), and Split
right, Split down and Close tab act on the tab the focused pane shows. Next and Previous are bound
to `DockHost`'s `tab_keys` (`DockTabKeys::CTRL`, Ctrl+Tab and Ctrl+Shift+Tab, by default; the
desktop passes Super+Tab so a program keeps Ctrl+Tab) at `KeyAt::Window` with `.capture()`: they
work from inside a text input and, as intercepted keys (guides/beui_keyboard.md), over an
embedded editor's view, which is how workspace-ui walks its tabs while an editor has the keyboard.
They are enabled only while the focus is in the dock: in its area (a `DockDialog` included), in one
of its floating windows, or on no node of its document, as when an app's view or a program's
window holds the keyboard. So a `Dialog`, sheet or menu outside the dock keeps them from switching
the tab under it, and of several docks in one document the one holding the focus takes the key.
A dock in another document (an editor whose dock stands alone, not joined) hears them only when
the shell's dock does not take them first: while the shell's focused pane has several tabs, the
shell walks those.
The dock binds no other key: the switcher and window actions are actions the shell binds.

## Modes

The dock picks its mode from its own size (`DockingLayout::mode()`; the workspace reads it to know
it is on a phone; thresholds are the styled `Docking`'s `NARROW_WIDTH`, `SHORT_HEIGHT` and
`PHONE_LANDSCAPE_WIDTH`):

- **Stacked** (`DockMode::Stacked`): the focused tab fills the dock under the dock's own stacked bar
  (icon and title, a count that opens a sheet of open tabs, back). A tab's `Pin::Title` action
  makes the title a button that runs it, with the action's glyph after the title as the cue for
  what a tap does (a pencil for Rename; a chevron when it has none; `dock.title.cue`). A joined editor's stage (the one
  bare pane of its group) is named and counted as the tab its dock took the place of
  (`DockState::standing_in`), so a document reads as itself. No tab bars, splitters or
  windows, but the whole state is kept, so widening tiles it again. Locks are ignored while
  stacked.
- **Gathered** (`DockMode::Compact`): a compact tree with a bare stage pane (an editor's stage,
  `DockState::stage_of`) keeps the stage and gathers every other pane's tabs into one foldable pane
  below or beside it. Its sizes are saved apart from the tiled ones (`DockCompact`), so switching
  sizes mixes neither.

## Menus and pinned actions

A tab hands the dock its menu with `dock_menu(items)`, shown behind More; actions marked `.pin(..)`
go into the bar. A horizontal bar too narrow for labels (a pane under `ROOMY_BAR`, or a phone's
stacked bar) leaves `Pin::Label` actions to More; an icons-only sidebar shows them as icons, and leaves one with no glyph to More. A menu is data, not a node, so a dock showing another app's tabs draws it from rows
it was sent (`ActionBuilder::detached`) and passes the pick back. `unstyled::ActionBar` does the
split between bar and More for any toolbar.

A pane's tab strip scrolls when its tabs do not fit. The face's `strip` gets the tabs and, as
`DockStripHandle::menu`, a row for every tab that shows it, with the tab's icon (the shown one chosen, marked at the row's end); the styled strip
fades the edge tabs lie past and, while any are hidden, ends in an All tabs button
(`dock.<id>.all-tabs`, rows `dock.<id>.all-tabs.tab.<tab>`) with that menu, since a fade ending on
the gap between two tabs shows nothing. Showing another tab, from that menu or a key, scrolls it into
view, and so does laying the strip shorter than before (its pane narrowed) when that hides the shown
tab: `DockTabBar` watches the strip's measured length, not on its first layout (whose reveal would
also scroll a page the dock sits in) and not as it grows, so a scroll the person made stays until a
narrowing hides the shown tab.

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
  dropped where no window is (its carry's `outside`) floats into a new window at the pointer.

`DockState::windows` is bottom to top, the focused window is on top, and `Document::layers()`
follows. The dock's drop site offers each window's places relative to that window's node, topmost
window first; the board chooses only places in the window the `DragPoint` names.

The window manager (`beui-dock-host/src/unstyled/hosting/manager.rs`, `snap.rs`,
`beui-dock-state/src/hidden.rs`) is on `DockHostLayout` for any shell: areas (`set_areas`, one `DockArea`
per display: the whole screen, where a window goes fullscreen, and the work area it is kept, snapped
and maximized in; a window belongs to the area it overlaps most), snapping, hiding and show-desktop
(a `DockWindow` declared `hidden` opens hidden and unfocused, until one of its tabs is shown),
cycling a `DockTab`'s `group`, and `window_actions(disabled)`, which returns `dock.window.*` and
`dock.switch.*` actions with no keys, acting on the focused window. The desktop binds them
(guides/desktop.md); the demo's Window manager page puts them in a menu.

`DockHost`'s `drag_modifier` makes the dock a window manager while those modifiers are held: a
primary drag anywhere in a floating window moves it, a secondary drag resizes it (or moves the
nearest split on a docked pane). Each window and pane claims those presses, so content never sees
them (`unstyled/modifier_drag.rs`; the desktop sets Super).

## Dragging, groups and locks

Dropping a tab on a tab bar inserts it where the bar parts for it (each bar is a `Reorder`), on a
pane's middle groups it with the tab the pane shows (`DockDrop::Group`; the carried tab names that
tab unless tidying would only fold it into the pane), on an edge splits it; Alt while dropping floats it. Dragging a pane's grip carries the pane.
Tabs and panes are carries on the drag board (guides/beui/components.md, Drag and drop): a bar's
row lifts its tab, the grip an `Interactive`'s `drag`, and `DockTiles` is a drop site
(`unstyled/fluid.rs`). At the lift, `State::spec` offers every drop the state admits as a
place (`DockPlace`), so a locked place is no place: for each pane laid out now, a split on each
edge, chosen in a band along it (`EDGE_ZONE` of the pane, each corner split along its diagonal by `Candidate::nearest_edge`; a pane hosting a group gives its edges
only `OUTER_EDGE`, then its group's places), and grouping in its middle, each zone leaving the pane's tab bar to the bar.
Each place carries the `DockState` its drop leaves and where the carried item rests in it.
A surface's places are reached only where it is the topmost of the dock's surfaces under the
pointer and nothing but the dock's own layers covers it (`open_where`, `State::topmost`), so a menu
over the dock, a tool-bar band or another dock in the window takes no tab; a dock dialog, modal
or not, covers no place (`modal` blocks only its parent panel's input). Over no surface of the
home window, or one that offers no place (the window a lone tab is carried from), a `custom`
place floats the tab in a window of its own at the pointer, which moves that lone window; with
Alt held that is the only place. A window the host opens is not the home window: a lone tab
carried inside its own host-opened window has no place there and goes home, as the host moves
such a window by its bar. A carry's payload is a
`DockCarry`, the dragged tab or pane with its dock's id, so another dock in the document, whose tab ids
may coincide, neither takes it nor hides a tab for it. Tool bars are carries of the same board,
with a drop site of their own (Dialogs, tool bars, fullscreen).

**From the keyboard.** A tab's row is a drag source whose tab button holds the focus, so a
focused tab moves with the carry keys (guides/beui_keyboard.md): Ctrl+Shift and an arrow move it
one place at once, along its bar, into the next bar or into the nearest of the dock's places
that way, and Move in the palette lifts it for the arrows to step between every place, each
said by its label ("right of Files", "with Files"), until Enter drops it. A split answers only
the arrow its edge faces (`step_toward`), so Right reaches "right of" a pane, never its top
band. Carried off its bar, its row keeps the focus (hidden and shrunk, not collapsed); dropped
in another bar or pane, where its row is built again, the pane that shows it gives its panel
the keyboard (`keyboard_after_move`, which lasts only for the drop's edit), as pressing its tab
would; so does a pointer's drop of a tab its press focused. Pane grips are not focusable, so a pane
moves only by the pointer.

**Splitters.** A splitter is a carry too (`SplitterCarry`), lifted at a mouse's or pen's press
(`lifts_on_press`) and past the threshold along its axis by a finger, and the drop site of its own
carry alone: a `free` place over its tree's canvas, unbounded, which turns the pointer's travel from
the press into a `SplitSize` as `SplitSize::moved` does. Unlike a tab it previews by writing: each
place chosen is written with `State::edit` from its site's own effect, and a drop writes its place
too (not `drag_edit`, whose write runs after input, before the board hears the pointer, so it
would reach layout a frame late). Escape or a
cancelled press puts it back to the size it had at the lift. Its carry `reads_outside`,
so past the window's edge it follows the captured pointer to its clamp, and a release there drops
it there. It scrolls nothing. Its arrows step it `SPLIT_STEP` of the
space as before, and a keyboard carry (Move in the palette, Ctrl+Shift and an arrow) steps it the
same; Space stays unbound.

**The preview.** Nothing is written until the release. While a place is chosen (or a tab bar parts
for a tab from another pane, `part_for_bar`), each tree canvas lays its panes and splitters out
as the place's state would (`State::previewed`) and springs every rect from where it is drawn
(`Parting`, on `DockCurves::settle`): a pane that would close shrinks into its edge away from the
neighbour taking its room, and a pane that only the drop makes is left open, empty. A pane's panel
keeps the size it was laid out at while the dock carries anything (`DockHeldSize`), clipped by
the pane as it moves, so no app or program in it lays out again until the drop commits; a panel
dropped into another pane moves there through its `Kept`, built once, and an app in it is placed
at its new size once, never unplaced on the way (a window resized mid-carry resizes its panels only then too); its
docked tool bars are the pane's and move with it. The carried tab is drawn the size of where it
would rest (`shaped`), under the pointer but inside its surface (`DockGhost`, the `preview` face with
`opening`), and glides into that rest on release. A pane keeps showing the tab it shows until the
drop. The carried tab is listed once: as its bar's preview while it floats, as the opening over a place.
A group is a tab holding a dock tree of its own (`Entry::Group`); the tree tidies itself after every
change (a one-tab group becomes the tab). A pinned group is never tidied away and admits only its
own tabs.

A `locked` pane makes an area, and every tab declared in it lives there (`DockState::home`). While
locked, a tab at home moves and splits freely inside the area but cannot leave or float; tabs from
elsewhere are kept out. Locks belong to places, not tabs. A drop the state refuses
(`DockState::admits`, `admits_leaf`) is no place: nothing opens there, and a release there
does nothing and the tab glides back to its bar; check `admits` first when a drop test does nothing.

## Nested docks and the host seam

Every `Docking` declares through a `DockHostLink` (`beui-dock/src/link.rs`), plain data both ways:
`DockDeclaration` up, `DockHosted` and `DockHostEvent` down, `DockRequest` up, `DockBodies` for the
host to place. The link comes from the `DockJoin` of the tab body the `Docking` is built in, else the
`DockHostSlot` context above; with neither it panics. A `DockHost` provides a slot that draws the
dock in place; a platform's slot (`DockHostSlot::new`) hosts it elsewhere, which is how a dock in an
app reaches the host across the app protocol (be-sdk-api PROTOCOL.md, "Docks cross apps").

A `Docking` inside a `DockTab` of another joins it and draws nothing. Built in a `Kept`, the join
lets the tab go while that subtree is hidden (`kept_shown`) and takes it back,
with its declaration, once shown and the tab is free; a workspace keeps a live editor this way
under a revision's preview. Nesting happens in the
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
  A bar of a tab that hosts a joined dock (its dock's own bars, which the joined dock lends that
  tab) shows on the joined dock's first pane until it is dropped on any edge of another of its
  panes, those of docks joined inside it included (`DockState::bar_panes`, `place_bar_on`, saved
  with the layout, and kept while a frame child parks it). Only a pane of the bar's own layer is
  saved: each layer's snapshot drops the docks joined inside it, so a bar left on a deeper
  layer's pane comes back on its joined dock's first pane. A bar stays with its pane, not with a tab: when the tab that names its pane leaves
  that pane (moved or closed), another tab still there names it; on one pane, bars of outer
  layers start nearer the edge, and a bar dropped among another layer's lands where it was dropped.
  Dragged by its grip (`unstyled/bars.rs`), a bar is a carry on the drag board, lifted by the
  grip's `Interactive` (`DockBarControl::carry`), and `DockTiles` is its drop site (`State::bar_spec`):
  `closest(band slots, sticky).when_far(float.thrown())`. Each band of each pane the bar may land
  on gives a slot at each gap of each run of bars and at its ends, read from the bars' laid-out
  rects, and is open within a bar of its edge (`DOCK_REACH`), held out to a bar and a half once
  reached (`DOCK_HOLD`); a bar grabbed floating over an edge reaches that edge's band only once
  pulled halfway from where it was grabbed towards it. The carried item is a square as long as
  the bar, centred on the pointer, so a band's slot is the one nearest along it (`following`)
  whichever way the bar lies. Away from every band it floats (`float_at`, a `custom` place): over
  the nearest edge of the pane under it (shown while carried only within two bands of that
  edge), or, over no pane it may land on, in a window of its own; let go at speed there it is
  thrown along its path first, never into a band, and not at all with motion reduced. The bands
  draw the chosen place (`Lifting::preview`): the carried bar's slot is a gap its size, and every
  bar slot glides (`Glide`) from where it was drawn as the place changes, so the bars beside a
  slot move aside; a dropped bar glides from where it was drawn into its slot. The bar is drawn
  by the dock (`Carry::drawn_by_site`, `DockBarLifted`), on a band's slot across the band and
  under the pointer along it, else under the pointer as grabbed; the carry is `kept_in` the
  dock's window, so it crosses the dock's own windows and leaves the window it was popped into.
  Panels keep resizing while a bar is carried (`DockHeldSize` holds them only for tabs and
  panes). The grip's menu button holds the focus, so a focused grip carries its bar from the
  keyboard between the slots of every band ("the top band of Files"). An app's grip (a hosted
  `DockBarControl`) sends `DockRequest::MoveBar`, which the root dock relays onto the board as a
  `PointerId::Relayed` carry.
- **Hidden bands** (`unstyled/reveal.rs`): a band of the dock's own bars along an edge the user
  auto-hid (`DockState::is_edge_auto_hidden`, saved with the layout) takes no room and is drawn in
  a layer of its own at `LayerLevel::Raised`, slid out from its edge by however far the owner says
  (`DockHostLayout::reveal`). Only a dock whose owner calls `offer_hidden_bands` (the desktop)
  shows Auto-hide in a dock bar's grip menu, and hides every band while a fullscreen tab or
  `set_covered` covers it. The owner hears each band's rect, whether it is covered and whether a
  bar holds it out (`hidden_bands`), and decides when to reveal it. A bar holds its band out while any
  overlay opened from inside it, other than an inert one, is open: the dock provides an
  `OverlaysWithin` (`beui-view/src/components/overlay.rs`) around each bar's content, so a menu, its
  grip's menu or a panel never has the band slide away from under it.
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
  the drag board then drops nothing and calls the carry's `outside`, unless the carry
  `reads_outside` (a splitter). This happens only
  when the host opens every window; elsewhere a drag out drops at the last point inside.
- `be_lib_ui_types::Input` has no window: it is also the app protocol's input, and an app region is
  always one window.

**Testing.** `testing::Stepped::hosting_windows` (be-sdk-test's `DocumentTest::hosting_windows`)
runs a fake host that opens every window at its size and reports back the next frame: `frame_in`,
`take_window_requests`, `report_window`, `windows()` and `snapshot_window(name, id)`.
`Stepped::hosted` runs under a real window-hosting layer such as `beui_layer_space::Space`, as
be-host-wayland's space test does. Dock window-manager tests live in
`beui-dock-host/src/unstyled/tests/`.

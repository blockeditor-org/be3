# Docking and windows

How to build a workspace of split panes, tabs and floating windows with beui's dock, and how the
dock behaves: stacked mode on narrow frames, dragging, groups, the switcher, dialogs attached to
tabs, and fullscreen. Read it before laying anything out as a dock. Overlays and layers in general
are in `guides/beui/components.md`; how a plugin's dock holds host panels and program windows is in
`guides/plugins/editor_api.md`.

`styled::Docking` is the workspace layout: panes split from one another, a tab bar on each pane,
and tabs that can be dragged between panes or out into windows floating over the dock.
`unstyled::Docking` underneath owns the tree, the dragging and the keyboard, and paints nothing.
The demo in `crates/beui-demo` is laid out in one.

## Declaring a dock

The children describe the layout the dock starts with and the tabs that exist; a
`DockingLayout<K>` holds where everything is now, and is what the caller saves and restores:

```rust
let layout = DockingLayout::new();
view! {
    <Docking layout home=Key::Files>
        <DockSplit id="root" direction=Direction::Horizontal fraction=0.25>
            <DockPane id="files">
                <DockTab id=Key::Files title="Files"><FilesPanel /></DockTab>
            </DockPane>
            <DockPane id="editors" empty={move || view! { <Nothing /> }}>
                <ForEach keys={open}>
                    {move |key: Key| view! {
                        <DockTab id=key title={title_of(key)} on_close={move || close(key)}>
                            <Panel key />
                        </DockTab>
                    }}
                </ForEach>
            </DockPane>
        </DockSplit>
        <DockWindow id="inspector" rect={INSPECTOR}>
            <DockPane id="inspector">…</DockPane>
        </DockWindow>
    </Docking>
}
```

- `DockSplit` takes two children; `fraction` is the first one's share. A split follows the code's
  `fraction` until the user moves it.
- `DockPane` holds `DockTab`s and `DockGroup`s (a tab holding a tree of its own, `pinned` to keep
  its tabs in it), names its starting `active` tab, and can start with its tabs in a sidebar
  (`vertical`, `sidebar_width`).
- `DockWindow` floats one tree at a `rect`.
- A tab's key is any `Clone + Eq + Hash` type. A key that is a `TabId` or a `u64` names the tab's
  `TabId` too, which the test ids of its close button and switcher row carry.
- Every container has an `id`, saved with the layout, so a layout saved by older code still finds
  its panes. A pane, split or window the code adds is placed beside its declared sibling when the
  saved layout lacks it.
- A tab's icon comes from the optional `icon` function, an icon-font glyph (empty for none) drawn by
  the tab bars, the drag preview and the stacked bar.

The children are the full set of tabs: a tab the code no longer declares is closed, and one it
starts declaring is placed.

- Closing is a request: `on_close` gives the tab its close button, and the tab goes when the caller
  stops declaring it, or stays (the workspace keeps a program's window until the program quits).
- A new tab joins the pane it was declared in, after the tab declared before it, and is shown. If
  the user closed that pane, it joins the declared tab nearest it that is still open, then a window
  the code declared it in, and failing both it floats in a window of its own.
- A pane with an `empty` view is never closed: emptied, it shows that view, and dragging its grip
  onto another pane moves its tabs and leaves it behind.
- A tab that stops being declared for a moment (a `Show` around it that flips, a `ForEach` rebuilt
  from new keys) loses its place, so keep a tab's declaration in one stable place.

## The layout handle

`DockingLayout` is a `Copy` handle owned by the scope that made it; `OwnedDockingLayout::new()`
makes one outside a scope (a host, or a test before it builds) and derefs to it.

| Call | Does |
| --- | --- |
| `snapshot` / `restore` | the `DockingSnapshot` to save, and putting one back; restore it in the same `batch` that declares its tabs, since a tab not declared when the dock reconciles is closed |
| `reset` | back to the layout the children describe |
| `show`, `focused`, `shown` | show a tab; read the focused tab; `shown` for a stacked dock |
| `replace(key, with)` | puts a new tab in an open one's place (same pane or window, same index) and shows it; declare `with` instead of `key` in the same `batch` |
| `state()` | the plain `DockState` value; `find`, `all_tabs`, `focused_tab` and `surface_tabs` answer from it |
| `enter_fullscreen`, `leave_fullscreen` | see "Fullscreen" |
| `begin_switch`, `step_switch`, `commit_switch`, `cancel_switch`, `switching`, `switch_choice` | see "The switcher" |

A list that opens what it names into one tab (the demo's catalog, be-launcher's pull requests)
replaces the tab shown last on a plain click; Ctrl (or Cmd) and a click opens another.

## Stacked mode

Tiled, `Docking` keeps its panes inset from its edges. `mode=DockMode::Stacked` draws the same
state as one screen: the focused tab fills the dock edge to edge, with no tab bars, splitters or
windows, and everything else in the state is kept, so switching back to `DockMode::Tiled` restores
the layout. The workspace stacks its dock on a phone.

- The dock draws the stacked bar itself: the tab's icon and title; a square counting the open tabs
  other than home, which opens a sheet of cards to show or close them; and a back button when the
  caller names a `home` tab (`home={Some(FILES)}`), which the back gesture also follows and whose
  own screen has none.
- What a tab adds to the bar is only its own actions, which its panel hands over while built with
  `dock_actions(node)` (shown while that tab is), and its menu, handed over with
  `dock_menu(actions)`, a `Memo<Vec<Action>>`.
- The menu is not tied to the stacked bar: a tiled pane offers the menu of the tab it shows behind a
  More button just before its close button, at the end of its tab bar or the top of its sidebar,
  and so does a window. A menu is a list of actions rather than a node, so a dock laying out tabs
  built elsewhere can draw it from rows it was sent, with actions made by
  `ActionBuilder::detached` (not registered for shortcuts or the palette), and pass the pick back.
- Each tab's panel is built once and moved between modes, so what it holds survives the switch.
- `recent_tabs` lists the tabs from the one shown last (part of the saved state); `stacked_tab` is
  the tab a stacked dock shows: the focused one, or the one shown last when the focused pane is
  empty.

## Panels

A tab's panel is built the first time the tab is shown and belongs to the dock, not the pane
showing it: the pane holds a `Portal` pointed at it, so the panel keeps its nodes, scroll position,
caret and state when hidden behind another tab, dragged to another pane or floated into a window.
A panel no pane shows is laid out by nobody, so it costs nothing and a screen reader does not read
it. Closing the tab removes it.

## Dragging, bars and keys

- Dragging a tab picks a drop target under the pointer: a tab bar inserts it between tabs there,
  the middle of a pane joins that pane, an edge splits it. Holding Alt while dropping floats it
  into a window, as "Pop out into a window" on the tab's menu does. A drag the state would refuse
  (`admits`, `admits_leaf`) shows no drop marker and does nothing on release.
- Every pane and window wears the same bar: a grip, the tabs, and a close-all button shown only when
  every tab in it can close. A middle click on a tab closes it.
- A finger picks a tab up by dragging it out of its bar, across the way the bar scrolls; sliding
  along the bar scrolls it.
- Dragging a docked pane's grip carries the whole pane (`DockState::drop_leaf`), with the same drop
  targets as a tab.
- A window holds one pane, so a tab dropped anywhere in one joins it rather than splitting it, and
  that pane's bar is the window's title bar: anywhere on it that is not a tab drags the window.
- Right-clicking a grip moves that pane's tabs into a sidebar beside its body
  (`DockState::set_vertical`), whose edge drags or arrows to a new width (`set_sidebar_width`); a
  window in that mode has no title bar, only the sidebar with the grip and close button at its top.
- Windows resize from any of eight grips and are raised by whatever takes the focus inside them.
  The grips and the bar between two panes reach `HANDLE_REACH` points past what they draw (a
  window's overlay is that much larger on every side, a splitter's item that much wider on both),
  so they can be grabbed without changing how they look, and they capture the press over whatever
  lies beneath.
- A split keeps each side at least `MIN_PANE_LENGTH` where there is room for both, and a window is
  drawn no larger than the dock, so a layout made on a wide screen stays usable on a phone.
- Ctrl+Tab and Ctrl+Shift+Tab walk the tabs of the pane the focus is in, registered with
  `on_shortcut` so they arrive even from a text input in a panel.
- The bar between two panes is a tab stop with a `Splitter` role; the arrow keys move it. The tab
  bar is a `Choice` inside a `Scroll` running the way the bar does, so arrows, Home and End walk it
  and scroll the tab they reach into view.

## Surfaces and window stacking

Each surface (the main one and one per window) lays its tree out over the rectangle it was given,
so panes and the bars between them are canvas items at computed rectangles, not nested boxes.
`layout_surface` is that calculation on its own, which drop targets and tests resolve against. A
window is a floating overlay, so it paints above the dock, takes the pointer where it is, and the
document underneath is shut out rather than answering a press through it.

How windows stack is the document's layer order alone (`Document::layers()`), not anything in
`DockState`. A press on a window and the focus moving into one raise its layer (over a fullscreen
tab too, unless the focus is in that tab). A drop target is looked for in the window whose layer is
on top at the pointer, never in one covered there by a layer that is not the dock's (a menu, a
dialog, the fullscreen tab).

## Groups

A place in a tab bar holds an `Entry`: a `Tab` or a `Group`. A group is a tab holding a dock tree
of its own, so choosing it shows that tree in the pane's body (one pane with a second tab bar under
the first, or panes split side by side), and the same drops work inside it.

- Dropping a tab onto the middle of another tab groups the two; onto a group, it joins the group.
  The outer edge of a pane showing a group still splits the outer pane, so the group's drop zones
  sit inside a thin band that belongs to its parent.
- A tab's menu offers "Group with next tab" and "Split with next tab"; a group's menu ungroups it,
  closes every tab in it, or floats it into a window as a whole.
- The tree tidies itself after every change: a group left with one tab turns back into that tab,
  and a tab bar left holding only a group takes the group's tabs, or its split, in its place.
- Reading: `entries`, `active_entry`, `locate`, `group_tabs`, `tree_leaves`, `surface_of`,
  `is_nested`; `layout_tree` lays a group's tree out as `layout_surface` does a surface's. `find`,
  `all_tabs`, `surface_tabs` and `show` look through groups, and showing a tab inside one selects
  the group in every bar above it.
- Changing: `drop_entry`, `group_with_next`, `split_with_next`, `ungroup`. `group_title` names a
  group in its tab.

A pinned group is never tidied away. `insert_pinned_group` puts one in a tab bar; it stays a group
while it holds one tab or none, cannot be ungrouped or closed from its menu, and admits only the
tabs it was given, so dragging any other tab onto or into it does nothing. Its tabs are pinned too:
one can be rearranged inside the group, and the group moved with all of them, but a pinned tab
cannot be dragged, popped or carried out with its pane. "Unpin from group" (`set_tab_pinned`) lets
a tab go; "Pin to group" pins it again once it is back. `unpin` makes the whole group ordinary.

`tree` reads a surface's or group's layout out as a `DockTree`, which names tabs but no leaf, split
or group ids, so it can be compared, sent elsewhere and rebuilt: `from_tree` makes a state from one,
and `set_tree` replaces a tree in place, moving in any of its tabs that were elsewhere and keeping
the focused tab focused.

## The switcher

The dock's switcher walks `recent_tabs` across every pane and window, as Alt+Tab walks a desktop's
windows.

- `DockingLayout::begin_switch(backwards)` chooses the tab shown before the current one (or,
  backwards, the one shown longest ago); `step_switch` moves the choice along and wraps;
  `commit_switch` shows the chosen tab (activating it in its pane and raising its window);
  `cancel_switch` leaves everything as it was. `switching` and `switch_choice` read it back.
  `DockState` has the same calls and `switch()`.
- While a switch lasts the dock opens an overlay in the middle of the document listing the tabs
  with icon and title, the choice highlighted, drawn by the `switch` render prop
  (`DockSwitchHandle`, a row per tab whose `pick` commits it). Nothing is shown until commit. A
  switch is not part of the saved layout.
- Escape cancels it, with whatever modifiers held and wherever the focus is (a program's window
  included): the overlay holds no focus, so the dock listens for Escape at the window while it is
  open and calls `Document::dismiss_overlay`. A dock in a plugin only hears a key a program has the
  keyboard for when it intercepts the chord, so linux-desktop also registers Alt+Escape as an
  intercepting action that cancels.
- The dock binds no other key. Whoever binds it picks the keys, usually a chord that begins or steps
  the switch while a modifier is held, and `held_modifiers()` letting go of that modifier to
  commit: linux-desktop uses Alt+Tab, the demo Alt+Q.

## Window-manager drags

`drag_modifier` (none unless asked for) makes the dock a window manager while those modifiers are
held:

- A primary drag anywhere in a floating window (content, bar or tabs) moves the window; a secondary
  drag resizes it from the edge or corner nearest the pointer (by thirds of the window).
- On a docked pane a secondary drag moves the nearest split; a primary drag does nothing (it is
  swallowed, since a press with the modifier is never meant for the content).
- Each window and docked pane claims those presses (`claim_modifiers`), so neither tabs nor content
  see them.

The code is `dock/modifier_drag.rs`. linux-desktop sets Super; the demo's Docking page has a switch
for Alt.

## Dialogs

A `DockDialog`, declared beside the dock's other children, is a window attached to a tab:
`<DockDialog id parent title size modal on_close>content</DockDialog>`.

- It hangs over the tab named by `parent` (or over another dialog, so dialogs of dialogs stack),
  centred on the parent's panel at the `size` its content asks for and kept on the parent's screen
  (`Placement::CenterOn`), in the `dialog_chrome` render prop (styled: a bordered card with a title
  bar and close button, `dock.dialog.<tab>` naming its body).
- It is not part of the `DockState`: never a tab, nothing drags it into the dock, a key declared as
  both a dialog and a tab is only a dialog, and it is not saved with the layout.
- It shows exactly while its parent tab does (`DockState::is_shown`), and rises with the window its
  parent is in, since its layer hangs from the parent's panel.
- A new dialog takes the focus. The dock tracks which dialog has it beside the focused tab: pressing
  a dialog gives it the focus, pressing the parent's pane or tab bar takes it back, and when a
  dialog closes the focus returns to its parent. The content's `use_dock_tab()` is a
  `DockTabControl` for the dialog, whose `focused` follows that and whose `show` focuses it.
- `modal` makes the dialog's layer `BlocksParent` until it closes or stops being modal. A press on
  the parent's panel is shut out and asks the dialog to close, which it answers by focusing the
  innermost modal dialog and flashing it (the handle's `attention`; `DockTabControl::attention`
  for content in another document, such as a program's window, to take the keyboard itself). The
  focus cannot move into the panel, and while the parent's tab is focused the focus goes to its
  innermost modal dialog.
- Escape or back with the focus in a dialog closes it through `on_close`.

## Fullscreen

One tab at a time can be fullscreen: `DockState::set_fullscreen` with a `DockFullscreen` naming the
tab and an area (a rect in document coordinates, or `None` for the screen it opens on),
`DockingLayout::enter_fullscreen` and `leave_fullscreen` from outside, or the `DockTabControl` that
`use_dock_tab()` hands the tab's own content.

- The panel moves into a floating overlay over that area, wrapped in Docking's `fullscreen` render
  prop (styled: the theme's background), covering tab bars, other panes, the dock's windows (even
  one the focus moves into) and anything else there; nothing under it hears the pointer, while the
  rest of the document stays live.
- Leaving puts the panel back in its pane without rebuilding it. Fullscreen is not saved with the
  layout.
- The dock binds no key to leave: whoever enters fullscreen offers the way out (the demo uses
  Escape). `DockTabControl::offer_fullscreen` adds Full screen / Leave full screen to the tab's
  right-click menu; only content with its own way out should call it.
- Showing or focusing the fullscreen tab (`DockingLayout::show`, the switcher, a press on its
  content) raises the fullscreen overlay above the floating windows, its own pane's window
  included, so a window brought in front and then left goes behind it again.
- The pane it left draws Docking's `away` render prop (styled: a note and a Leave full screen
  button), seen where the fullscreen area does not cover it, such as another screen.

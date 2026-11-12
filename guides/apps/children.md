# Placing children

An editor puts another block's editor (another app, any number of levels deep),
a program window, a host panel or a picker inside its own view. The host runs
the child and composites it; the placer only reserves the space and says how
the child should behave.

## Subregion and ChildBlock

`<Subregion placed={Some(content)}>` reserves a rectangle and asks the host to
fill it with a `SubregionContent`: a block's editor, a web view, a host panel,
a program window, a template being made, an artifact's settings, a picker or
a rename form. `<ChildBlock>` is the `Subregion` for a block. It reserves its
space with beui's `Embed` node, and the framework turns the rect and clip
that node was laid out in into one placement per frame.

The subtree between `ChildBlock`'s tags is drawn inside the rectangle and is
handed a `ChildBlockHandle` whose `ReadSignal<ChildState>` says what the host
made of the child: whether it is available (a block already open above this
editor is refused, with `error` saying so), its intrinsic size, aspect ratio,
manifest `interaction`, capabilities and resize rules, and for a program
window its title, icon and attention counters. A placeholder is a frame
whose visibility follows that signal, not a branch in the view. `on_state`
reports the same to the editor around it (how a stage sizes itself to a
slide's aspect ratio).

The host runs a bounded number of children as editors and draws the rest as
previews, so place only the children that are visible.

## Drawing order

A child is drawn under the app's own surface with its space cut out, so
anything drawn after it (a handle, a drag ghost, a menu) covers it. A
translucent child cannot composite that way; place it with
`layer=ChildLayer::Above`, which composites over the app's pixels and cannot
be drawn over. An overlay of the editor's covers children painted beneath it
for drawing and for the pointer, but not the children inside it.

## Modes

- **Passive** (default): the editor keeps every click. Promoting it to
  `ChildMode::Active` (a double-click, an Edit button) hands it input; the
  host takes that away on a click outside or Escape. A promotion made during a
  press that began over the child replays that press in the child from where
  it went down (`be-host/src/compositor/app_view/handoff.rs`); a press already
  released is replayed only within half a second.
- **Live**: the child is the whole of what the editor shows (the slide a
  presentation edits), live from placement, never taken away.
- **InPlace**: live from placement but never takes the frame and draws no
  chrome. With an intrinsic size it keeps its layout size and is rendered at
  the placed rect's zoom, so it grows with a canvas's camera without laying
  out again. A canvas places a checklist this way, and an image or another
  canvas passive.
- **Preview**: the host draws the block's preview rather than running its
  editor (a filmstrip). A block previewed in several places at once is drawn
  once, at the largest, and each place shows its part.

`own_frame=true` gives a child a frame of its own inside its rectangle, where
it draws its own toolbar and sidebars while the placer keeps its whole frame.
With a `top_bar` it also offers its parent the frame's menu; that is how every
workspace tab gets one and the file pane does not.

## A child that takes the frame

An Active or Live child takes the frame over: it is handed the frame the
placer had, draws every chrome band, and draws its content at the rect it was
placed at. The placer draws no chrome while that lasts and its content takes
the whole frame; its camera is untouched, so the child's rect stays where it
was on screen. An editor cannot keep a band while a child has the frame.

Clicks on the child's chrome belong to the child. A click on the rest of the
frame, or on the child's content outside its placed rect (which is where the
parent's world shows; `editor.placed_content()` tells the child that rect so a
canvas clips there), falls through to the placer and takes the frame back,
as do Escape and the framework's exit.

When the placer's dock has joined the root, the child's bars join the pane of
the tab whose frame it took, in place of the placer's bars, which come back
when the child lets go (`DockDeclaration::splice_bars`). Neither editor writes
code for this.

An editor that keeps its own chrome while a child is edited places it with
`own_frame` in a tab of its dock instead. The presentation does this: a
locked filmstrip, a stage tab whose body places the slide with `own_frame`,
a notes tab under it placing the slide's notes block the same way, and its
toolbar as a `DockBar` lent to the slide canvas's pane.

## Focus

When the host gives the keyboard to a child, every instance above it hears
which of its children it went into (`ChildBlock`'s `on_focus`): a press inside
a child never reaches the placer. A `ChildBlock` or `Subregion` in a dock tab
shows that tab when it hears this (`DockTabControl::content_focused`, so a dock that
stops showing the tab hands the keyboard to the one shown in its place), and one in
an empty pane's view focuses that pane, so a press in a child (a joined dock's tab body or empty pane included)
focuses what holds it and brings its window in front; `on_focus` is for
anything more. A child that takes a press without the
keyboard (one being presented) brings nothing forward. The other way,
`EditorHost::focus_child` gives a child the keyboard, but only while the
keyboard is already inside the editor's views and no popover or
keyboard-taking layer is open, so it never steals focus from elsewhere. `ChildBlock` and `Subregion` in a dock tab ask for it whenever the
dock gives that tab the keyboard. `EditorHost::focus_block_child` asks the
same for the child that will show a block, so an editor that makes a block to
type into ("Add notes") asks in the frame that makes it, before the child
exists.

While a gift is pending, be-host holds the keys typed into the editor's view
(`be-host/src/compositor/app_view/held_keys.rs`). Every gift ends explicitly,
never by a timeout:

- Granted when the child's view takes the keyboard. The child's app hears the
  held keys, in order and ahead of keys typed into it since, once a text field
  in it has the keyboard (its view asks for an input method), or, for a child
  with none, two frames after it had its block's content and the focus (the
  count starts again if the content is replaced).
- Refused when the person presses in the editor or takes the keyboard
  elsewhere, and when the child or the editor goes or its app stops. A block
  gift waits for a child to show its block however many frames that takes: the
  editor has to change its content first, which a busy host can delay. It ends
  at once when the host refused to make the block, when the editor has no view
  left, and when the editor asks for another block (only the latest ask
  stands). The editor hears the held keys if it still has the keyboard;
  otherwise they are dropped.

Only children placed in a mode that runs an editor count; a gift to a child
that is placed but never shows a view that takes the keyboard (a view behind
another that holds it), and a block gift whose block is only ever shown as a
preview or hidden, last until the person presses or moves the keyboard.
Keys typed into a view right after a press there wait the same way until its
app has drawn a frame after the press (only for a press the app was sent, as it
draws no frame for one that never reached it), since that frame may carry the ask
("Add notes" asks as it is clicked), and go to the view if it asked for
nothing. Owed keys and the keyboard are kept by view, not by the node that
catches its input: when the host builds a view's catcher again (a late child
placed in a new pane), the new one takes over both, unless the keyboard went
elsewhere meanwhile. A view that is no longer placed leaves nothing behind.
Held keys skip the child's input method, so a composition (CJK) started before
the child shows arrives as its committed text. An empty text block given the
keyboard puts its caret in its text.

## Docks across apps

An editor whose layout is a beui `Docking` placed inside a tab of another
app's dock joins that dock, as nested docks do in-process
(guides/beui/docking.md, Nested docks and the host seam). The protocol is in `crates/be-sdk-api/PROTOCOL.md`
("Docks cross apps"); be-sdk-beui does the app's part and the editor writes
nothing but its `Docking`.

- An instance whose frame is placed docked declares its dock
  (`EditorHost::declare_dock`) instead of drawing it. Each tab body is built
  where the host shows that tab's view, with that view's window as
  `use_window()`, so its size, key scopes and the menus it opens are the
  tab's.
- The root hosts each child's declaration as a pinned group in place of the
  tab. `be-host-runtime/src/docks.rs` routes docks between instances,
  applying a commit's child placements before its dock messages.
- The tab body holding the editor's content is where the host keeps its
  camera, chrome and frame takeovers, so a joined canvas pans and hands its
  frame to a card as it does alone.
- A child keeps its child id while its body moves between views, and stays
  docked while its placer hosts it, after (`DockHold`), and while the placer
  is off screen, so it is never rebuilt undocked in between.
- Each docked editor's own layout is saved by be-sdk-beui
  (`docked/saved.rs`) in this device's `DisplaySettings`, keyed by block type
  and dock, and restored whenever the setting changes, so every open
  instance follows the last one moved. A dock that does not join is wrapped
  in a `DockHost` and kept under the same key (`DockKeeper`), so a bar moved
  in one place shows there everywhere. An editor that saves its own dock
  (be-sdk-shell's `Workspace`) provides `DockKeeper::none()`.

## Program windows and host panels

The host's debugging panels (`HostPanel`) are drawn by the host but placed by
the workspace as dock windows holding `SubregionContent::Host(panel)`.

Program windows are placed the same way. On Linux the host is a Wayland
compositor (guides/desktop/wayland.md); `editor.watch::<WindowsWatch>()` lists
its windows, answered only to the shell; any app may watch `OpenProgramsWatch`, the programs
with windows open, without the windows. be-sdk-shell's `Workspace` gives each
top window a program tab holding `SubregionContent::Window(id)`, and a window
with a parent a `DockDialog` hanging from its tab. The rest (title, icon,
raise, fullscreen, attention) is the child's `ChildState`, reported only while
placed, so the workspace keeps what each tab last heard. Closing a tab is
`close-child`, which asks the program to close; the tab goes when the window
does. A modal dialog keeps input off its parent on the host's side too.

The desktop's `DockHost` uses Super as its drag modifier, so Super+drag moves
a floating window even over a program. For that the press must reach the app:
be-sdk-beui reports the document's press claims with every frame
(`FrameReport::claims`), and the host's `AppView` claims those presses before
any child under them (guides/beui_keyboard.md, One input path). A client's own
`xdg_toplevel.move` and resize requests are not honoured.

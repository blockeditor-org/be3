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

The host draws a notice in place of a child it cannot show: no access, a key
not shared yet, or, offline, a block this device has never read
(`BlockInfo::out_of_reach`), which opens once the host reconnects.

The host runs a bounded number of children as editors and draws the rest as
previews, so place only the children that are visible.

Under be-host's own UI, a block placed in several places at once shares one
editor instance, the one
under the block's own id that the host's own requests (focus, menus, renaming a
new block) reach, so a shared instance draws one view for all of them. A
placement that asks for another editor than the one holding the block (a
settings page in a tab beside the settings list) gets an instance of its own,
a twin under a key of its own in be-host's `Editors` (`compositor/editors.rs`;
`Editors::block` maps it back), closed when that placement goes. Previews are
never twinned. Under a root each place gets a live editor of its own instead,
the rule going forward (below).

## Under a root

Under `--root` (plans/host_as_platform_adapter.md, 4g7b3) the root shows an
embedded editor's children, not be-host's compositor. be-sdk-root's `Children`
reads each view's placements from the app's layout commits
(`Embedded::placed`) and shows each block child placed to run its editor as
that block's editor, which the host opens and serves as it does the root's
other editors (`be3:root`'s `host.open-editor`, and `host.close-editor` once no
place shows it). Each staged app runs once for every child, holding every
editor of it the root shows, each asked for screens of its own; the app
showing the session's block is not one of them. Each place gets a live editor
of its own: a block placed twice is edited in each place, as two editor views
of one open block each get their own, and what one changes reaches the other
through the block. Its screen is the size of its place, at its
placer's raster, composited under the placer's layer (over it when placed
`Above`), and the placer is hit everywhere but where a child that takes input
is. The root tells the host through `be3:root`'s `host.show-children` which
editor shows each child, and the host makes each child's status from that
editor's manifest and tells the placer, as it does for its own children.

The keyboard follows the same rules as under be-host (Focus, below): the root
reads each editor's `keyboard-ask` from the placements it settles and, once the
frame's layers are drawn, moves the keyboard into the child's layer, or out of
the placer's screen until the child shows (`Children::keyboard`), while
nothing outside the editor took it meanwhile. Placers hear
`child-focused` through `be3:root`'s `host.tell-placer`, and a press outside an
active child takes it back (`Children::revoke`); an Escape the child leaves to
its placer does not, so the keyboard stays where it is.

A child that takes its placer's frame (below), the first that could in the
placer's content view and only once it shows its screen, is asked for a screen
the size of its placer's, with the placer's frame drawn and its placed rect as
the frame's `content` (`framing_screen`), and shown over the placer, which is
asked for its view with no chrome meanwhile (`Children::withdraw_chrome`). It
is hit everywhere but where its content shows outside its placed rect, so a
press there reaches the placer and takes the frame back, as Escape in the
placer does, from every child of that view that could take it.

Docks join as under be-host (Docks across apps, below): the outermost
`Children` holds one be-sdk-app-host `Docks` for every editor it shows, hears
what each app's docks say once its placements are in (`Embedded::take_docks`),
and hands the root's host what the docks have for each editor through
`be3:root`'s `host.dock`, which the host's `Forwarded` answers the editor's
dock watch with. A child placed in a dock tab's body is asked for its frame
`docked`, and stays open while the dock hosting it keeps it, placed or not. The
bodies of a joined dock's tabs and bars the outermost dock places
(`ChildContent::DockTab`) are shown as the owning editor's views, each another
screen of its app placed `Dock`, framed where it holds the editor's content.

A child placed as a preview runs its editor too, asked for its preview view the
size of its place (`preview_screen`, capped as be-host caps a preview), hit
nowhere and shown under its placer. The host tells the placer it composites
it (`ChildPreview::Composited`), so be-sdk-beui leaves a hole for it as for
any child rather than drawing a picture; one whose editor draws no preview runs no
app, and its placer draws the block's label (`ChildPreview::Label`). Promoted,
it keeps its editor, asked for its main view instead. A child placed in a
preview view is a preview too, and an editor shown only as a preview has no
dock, as under be-host.

A block a root's editor opens (`open-block`) goes to the editor that handles
show requests, as under be-host: the nearest of the asking editor and those
placing it, which the root names in `host.show-children`, whose manifest
accepts them, else the editor showing the session's block. The root tells
every app it embeds the host's catalog (`be3:root`'s `host.block-types`),
which names block types and draws their icons. A child placed by its parent
without a frame of its own keeps no camera, as under be-host (guides/pan_and_zoom.md,
Who owns the camera): the root tells the host where its content shows and the
size it wants (`host.place-content`), and the host hands a pan-and-zoom editor
its view from them.

A child is drawn where it was placed as under be-host: turned by its
`rotation` about the middle of its place and faded by its `opacity` (its own
children with it, since each layer is placed through its placer's transform).
One placed with an intrinsic size, whose editor does not pan and zoom, is laid
out at its place's size over the zoom that fits that size in it and drawn at
that zoom (`child_screen`), and the host tells its editor that size
(`host.assign-size`, the `resized` event be-host sends). One placed to `fit`
its content is asked for at most that room, drawn from its place's top left
clipped to its place, and its placer hears the room its layout said it used
(`layout-commit.sizes`) as its status's `intrinsic`. A view's occluders give the placer the presses over the children
placed before them; they hide a child placed below its placer only because
the placer draws over it there.

What an editor tells the shell reaches the root, which acts on it as be-host
does: its menus go to the docks and to the status of a child whose frame it
draws (`shown-child.menu`), an ask for its details goes to the docks or its
placer, a pick from a child's menu goes to the editor drawing that child's
frame, and `leave-frame` gives the frame back. A block an editor drags
(`drag-block`) is carried by the router of the window its press is in
(be-lib-input-router's `start_drag`) over the views under the pointer that take
a drag, each told through its host (`host.tell-editor`, `editor-event.drag`),
dropped on release and put down by Escape.

A picker, a rename form or an Export dialog an editor places is a dialog the
root opens, like a block's editor, in the editor that runs it (`be3:root`'s
`shown.dialog`), its placer's own; what it tells the shell that its placer's
status says (its pick, its answer) the root relays to the host
(`host.tell-shell`). It is available at once, as under be-host, and registers
no dock. A creation child is opened the same way, in the template's editor:
the placer's `commit-child` reaches the host through the root, which makes the
block once the editor is open, and the child's status says how far it got. An
artifact's settings open in the editor that exports its source, at its
settings view; their commit applies the draft to the artifact.

The root hands every view an anchor space (`screen-request.anchor-space`): the
session's screen anchors anywhere in the window, and a child hands on its
placer's, moved to its place and scaled by its zoom. So an editor's menus,
popovers and sheets open as anchored views (guides/apps/framework.md, Anchored
views), which the root shows over everything the session places, at the rect
the app reported (`Embedded::anchored`, `Children::anchored_layers`), each a
placer of the children placed in it, as a popover places a picker, stacked in
the order they opened across apps. One that asks for the keyboard gives it to
the child placed in it once that shows, else the screen of the view it hangs
from keeps it and hands its keys on; that screen takes it back once the view
closes. A press closes what be-sdk-app-host's `anchored::dismissed_by_press`
says, as be-host's compositor does, where each view takes presses
(`frame-report.floating`), told to the app as `input-batch.dismissed`.

Creations, artifact settings, host panels, web views and program windows
(plans/host_as_platform_adapter.md, 4g7b3g) are not there yet: a child of
another kind the placer hears is unavailable.

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
was on screen. Clicking or focusing a child never moves or resizes it, even
one zoomed to fill the view, and neither does a side panel opening, closing or
moving around it: a panel covering part of it keeps covering it, and the
person zooms out to see it. An editor cannot keep a band while a child has the
frame.

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

When the host gives the keyboard to a child, every instance above it hears which
of its children it went into (`ChildBlock`'s `on_focus`): a press inside a child
never reaches the placer. A `ChildBlock` or `Subregion` in a dock tab shows that
tab when it hears this (`DockTabControl::content_focused`, so a dock that stops
showing the tab hands the keyboard to the one shown in its place), and one in an
empty pane's view focuses that pane, so a press in a child (a joined dock's tab
body or empty pane included) focuses what holds it and brings its window in
front; `on_focus` is for anything more. A child that takes a press without the
keyboard (one being presented) brings nothing forward.

The other way, an editor asks that a child take the keyboard in the child's
placement, never by a call: `ChildBlock` and `Subregion` take a `keyboard`
count, which only grows, and each time it grows above the highest it reached
the placement carries the editor's next `keyboard-ask` serial (be-sdk-beui's
`register_child`). A `ChildBlock` or
`Subregion` in a dock tab adds one each time the dock gives that tab the
keyboard or brings its window forward, while no popover or keyboard-taking
layer of the editor's is open. An editor that makes a block to type into ("Add
notes") places its child with a count of one once it places it; a program tab
counts its window's raises and calls for attention.

The host acts on the layout commit that carries a serial above every one it
acted on for the editor (be-host-runtime's `Instances::hear_keyboard_asks`,
which names the child to the compositor with `InstanceEvent::KeyboardAsked`,
one an editor per commit; a restarted app's editor counts from 1 again). The
ask ends without moving anything if, at that commit, a view that takes the
keyboard is open or the keyboard is outside the placer's views and the
children they show (`ask_at_commit`); an ask still waiting ends when its child
is no longer placed (`KeyboardAsked(None)`) or its app fails. A
child whose view is up takes the keyboard at once, if it is inside the
placer's views or the children they show and no view that takes the keyboard
is open, so it never steals focus from elsewhere (be-host's
`focus_when_given`). The view that takes it is the one showing the child's
content (`asks::acts`): a joined editor's content shows in a dock tab body the
placer places as a child of its own, beside its bars, so an ask for the joined
child, its tab or one of its bars lands in that body, never in a bar or the
main view left placed and shown nowhere. For a child whose view is not up yet (not built, or its
app has not laid it out), the keyboard leaves the placer's own view at once
(`keyboard_into_child`; one already in another of its children stays there),
and the child's view takes it once it is up, while nothing outside the
editor took it meanwhile. Keys typed in between reach nothing. An app that does not run yet
still hears which view has the keyboard (`Input::Focus`), so it focuses the
field once it runs. A window child (a program window) takes the keyboard from
the host directly.

Keys are never held (decisions/keys_are_never_held.md): a key goes to whatever
has the keyboard when it is routed, and one routed to a child whose app does
not run yet is dropped, as is everything an app that is not running would hear
but where the keyboard is.
Nothing waits for an editor to claim a key or the keyboard.

The keyboard is kept by view, not by the node that catches its input: when the
host builds a view's catcher again (a late child placed in a new pane), the new
one takes the keyboard back, unless it went elsewhere meanwhile. A child never
reads what its placer's view stores (`Placer` names the app with a plain
string), as a child's effects can outlive the placer's view.

A joined editor's content moves from its main view, which stays placed but is
shown nowhere, into its dock tab view, built after the main view goes. That
view takes over the keyboard the main view had (`keyboard_follows_content` in
be-host's `app_view.rs`; the runtime's `content_moved` finds where the content
moved).

An empty text block given the keyboard puts its caret in its text. It reads its
own views' keyboard (`Editor::has_keyboard`), not the document's
(`use_document_keyboard`): an app's instances share one document, and an
instance reading the document's would act on a keyboard given to another.

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
  tab. `Docks` (`be-sdk-app-host/src/docks.rs`, which be-host-runtime holds one
  of) routes docks between instances,
  applying a commit's child placements before its dock messages.
- The tab body holding the editor's content is where the host keeps its
  camera, chrome and frame takeovers, so a joined canvas pans and hands its
  frame to a card as it does alone.
- A joined editor's tool bar (`DockBar`) shows in the root's band at the size
  it measured, squeezed along its run where the band is shorter (a phone's),
  so its surface scrolls instead of being cut off.
- A joined frame shows no main view of its own, so its dock tab views carry
  `FrameSpec::phone` too: on a phone it lays out, and opens Rename as a sheet,
  as an editor on its own does.
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

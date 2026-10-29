# Block plugin protocol

The host and a plugin speak the `be3:plugin` WIT world `editor-plugin`
(`wit/`), carried by be-ops (guides/type_modules.md, "Operations and WIT"); the
Rust on both sides is generated from it into this crate. The world imports:

- `be3:gpu/device` (crates/block-gpu-abi/wit), the device the plugin draws with,
  answered inside the plugin's own worker or thread;
- `host` (`host.wit`): the handshake (`connect`, the plugin's first call), what a
  frame tells the host besides itself (`layout`, `frame-needed`,
  `missing-characters`) and the watches of preferences, the UTC offset, fonts
  and input;
- `blocks` (`blocks.wit`): the blocks an editor reads and changes, its replicas,
  the graph, history, heads, artifact states and version control;
- `shell` (`shell.wit`, with its shapes in `shell-types`): what an editor tells
  the shell around it, its menus, children, audio, web views and creations;
- `host-services` (`host-services.wit`): what an editor asks of the host and
  waits for, files, revisions, access, fetches and staged data;
- `account` and `contacts` (`account.wit`): whether the account administers its
  company, the base of block links, where preferences come from, and the
  account's contacts, which the host answers only to the shell;
- the interfaces of `be3:desktop` (`wit/deps/desktop`), an extension
  beside the core protocol rather than part of it: what a host running a desktop
  (a Linux session, or Windows) offers the editors that draw it, one interface
  per part of the desktop (windows, input devices and remaps, displays, power, media, Wi-Fi,
  e-paper, notifications, the screen lock, programs, the tray). A plugin's
  manifest names the extensions it uses (`extensions`); the host answers a call
  into one it did not name `denied`, and a host without a desktop answers its
  watches `unsupported`;
- `be3:debug/debug` (`wit/deps/debug`), the debugging extension, beside the core
  protocol too: performance measurements, simulating a lower access to a block,
  and which catalog templates are for debugging (the picker's Debug group). The
  world also exports its `paintings`, what the plugin painted for the host's
  tests. A host without these tools answers and asks none of it.

Ids and access levels come from `be3:shared` (crates/be-ops/wit), which the
type-module world and `be3:gpu` use too, so an id has one shape everywhere: a
uuid is its 16 bytes and a hash its 32, each under the alias of what it names
(`block-id`, `block-type-id`, `commit-id`, ...), and a number one side hands
out is an alias named `<thing>-id` (`screen-id`, `child-id`, `web-view-id`).

It exports `plugin` (`plugin.wit`): `resource editor`, one open editor instance,
whose methods are what the host says to that instance and whose drop closes it;
the screens and their configure, `draw`, whose answer is the frame,
description, and shutdown. What an editor asks for a block it draws goes two ways: `blocks.act` with a
`block-command` changes the block (undo, redo, its artifact, unlink, delete,
move, place, restore), and `shell.act` with a `shell-command` asks the app
around it (share, leave, close the editor). Shapes shared by several
interfaces are in `types.wit` and `accessibility.wit`, which mirrors AccessKit.

Every call a plugin makes about an editor instance is made under that editor's
resource, so the host knows the instance from the call's parent and closing an
editor ends everything it opened. A value the plugin follows is a resource
whose `next` answers with the latest value once it changed; a change made while
the plugin does not ask is kept, folded into the ones after it, until it does.
One named `*-watch` with a `constructor()` and `next` and nothing else is a
latest-value watch, which be-ops-bindgen implements `be_ops::Watched` for, so
an editor follows any of them with `editor.watch::<W>()`.
Input is one of them: batches for one screen that the plugin has not taken yet
merge by the rules beui queues a frame's input by: each pointer's moves fold into
its latest unless the screen's window requests ask for every sample, scrolls and
zooms fold into the last of a run, and everything else is kept in order. A watch answered
with a list of events has a merge rule that keeps it bounded: show requests keep
each request once, at its latest place; web-view events fold a title, address or
failure into one of its own kind just before it and keep at most 256.

The host handles the calls a plugin makes in the order it made them, whatever
interface each belongs to: one heard while the host draws a frame waits for the
next frame together with every call after it, so a `create-block` is done before
a `read-plain-file` or `fetch` the plugin made after it. Only what a frame
itself carries (its layout, `frame-needed`, paintings) is taken at once.

The accepted handshake carries the theme the host is drawn in, which is what
a plugin's own interface follows, so an editor looks the same as the app it is
embedded in wherever it runs.

The protocol has no version and the handshake checks none: the app and its
plugins are built together, so a change to a function needs no bookkeeping. A
call the other side does not know is answered `unsupported`. The plugin checks
the order of what the host asks of it (no screens before the handshake, input
only for screens it was given, an editor opened once and closed once) and stops
when it is broken; the host gives a plugin a few seconds to say hello and to
shut down, and stops one that takes longer.

Input events carry pointer, touch, wheel, key, text, focus and zoom-gesture
input in the screen's own logical coordinates. Touch input preserves the
device, finger, lifecycle phase and optional normalized pressure. The host
also sends the primary-pointer events synthesized by its window integration;
a receiver that handles touch directly suppresses those duplicates. A pinch
or trackpad zoom is its own event, carrying the factor the view is asked to
grow by; a wheel turned with the zoom modifier held stays a wheel event, which
the receiver reads as a zoom of its own. Consecutive moves from one touch
coalesce to their latest position and pressure. Consecutive zoom gestures
coalesce by multiplying their factors.

A press made in one screen keeps that screen's input while it is held, as an
operating system keeps a press with the window it began in. While it is held
and the pointer is over another screen of the same plugin, each pointer event
names that screen (`over`) and carries its position in that screen's
coordinates, so a drag carries across a plugin's screens; over none of them the
host sends `pointer-left` once and goes on with positions in the holding
screen's coordinates, outside its rectangle.

A back gesture - the system's back button or swipe, or a Back key - is an
input event of its own, carrying its phase: started from an edge, its
progress, cancelled, or invoked. It is not sent to the screen with the focus.
Each frame report says whether the instance has something to go back from on
that screen, and the host sends the gesture to the last screen it drew whose
report said so, unless the host's own interface takes it first. With no such
screen the gesture is left to the platform.

A screen's input goes out at the start of the host frame that received it,
before the host lays anything out, and it is routed against where the screen
sat and what covered it the last time the screen was drawn. The host has that
much of its own frame left to spend, so a plugin answering promptly is drawn
into the frame that delivered the input rather than the one after it. A screen
the host did not draw last frame is sent nothing.

Opening an instance carries the account and organization its block client speaks
for, and the id of the app client the host itself is: a per-installation
identity a block that stores one setting per client - the settings block -
resolves against, which the instance's own tunnelled client cannot supply
because the host opens a fresh one for every runtime.

An editor instance reaches the content of its blocks through `blocks`'
`replica` resources: frames the host relays unread, in order and
back-pressured both ways (a stream window to the editor, a window of unanswered
`send`s from it), between the instance and the block's replica
(guides/the_new_block_stack.md, "Content on the plugin protocol").

A plugin may edit several block types, and every opening of an
instance names the block type whose editor it is for, which is how the plugin
picks the editor to run: the block's own type, the type of the editor whose
template is being made, or the type of the block an artifact was generated
from.

A manifest may declare an editor for every block type (block type "any"),
which the plugin runs for any type no other editor of its own names. One block
type may have editors in several plugins; the host's catalog lists each editor
by its id (`editor_id` of its plugin's id) with the block type it edits, or the
nil type for every type. A child placement, a request to open a block and a
request to show one may name an editor; the host opens the block in it when it
edits the block's type, and otherwise in the type's built-in editor, the first
plugin found that names the type.

Every block is made from a template the plugin's manifest declares, and an
instance opened to make one is opened as a creation instance instead of on a
block, naming the template it makes: it has a client of its own but no block,
and a template the manifest marks as a dialog is also shown as a dialog, in
which it asks what it needs first. When the user accepts it the instance asks
to close with `close-dialog(accepted)`; the host then asks it to commit, and
the instance creates the block through its own client and answers with the
block's id, or with why it could not be created; the host then opens the block
it was given.

An instance shown as a dialog (its frame's `dialog` is set, which an editor
asks for by placing a child with `dialog` set) gets the whole area inside the
card or sheet, with no padding and no title bar of the host's: it draws the
title, the content and the actions, and reports the height it wants as its
content (the host passes the size it got as the frame's, and how much of its
bottom is covered as `bottom-inset`). Escape, back and a press outside it go to
the instance first, so a form with unsaved changes can ask before closing: the
editor showing it calls `dismiss-child`, the host calls the instance's
`dismiss`, and the instance answers with `close-dialog`, which reaches the
editor showing it as the child's `dialog-status`. Whether an instance answers
is decided up front rather than by waiting: it says `handles-dismiss(true)`
while it can, and until it does the host answers a `dismiss-child` for it at
once with `dismissed`, as it does for an instance that stopped.

An editor whose block type generates dynamic artifacts is also opened on an
artifact: an instance with a client of its own, the artifact's block and the
opaque settings the host stores on it. The instance answers with the block the
artifact was generated from and a short description of what the settings
produce, or with why they cannot be read, and repeats that answer whenever the
settings change. The host sends the settings again whenever it changes them
itself; the instance's settings view edits its own copy and reports every
edit, which the host stores on the artifact only once the user applies it. A
regeneration request carries the settings to rebuild from and is answered
exactly once, after the instance has written the artifact through its own
client, with success or with why it failed.

Every block id a call carries is in the instance's own scope. An instance
whose block sits inside a version control checkout names the blocks of that
checkout by the ids their content stores, which are the same in every checkout
of the same history, and the host translates them to and from the blocks it
actually holds at the boundary, in both directions and for every call. A
plugin never sees the translation and needs nothing to take part in it.

An editor of a repository or a checkout asks the host to act on it with a
version control command naming that block: adopting a block as the tree to
version, saving the working change with a description, bringing the branch's
changes in, switching, making a branch or a checkout, combining another branch
in, resolving a conflict by taking one side or keeping the block as it is,
undoing any recorded operation, and forking, pulling or pushing an upstream in
the same organization. The host answers an instance watching the status of
its block, when that block is a repository or a checkout, whenever it changes: whether
it is busy, the last error, the branches, the working change and what it
changed since its parent, whether the branch has moved past that parent, the
history, and the operation log.

An editor instance may ask for the cursor shown over one of its views,
which only the host can put on the window. The request names what the cursor
means rather than any one toolkit's spelling of it, is sent only when the
cursor changes, and is honoured only while the pointer is over that view.

An editor instance may report the area its text input occupies and where its
caret sits inside it, in the view's own logical coordinates, which is what an
operating system's input method needs to place its candidate window. Only the
host can tell the window that, and it maps the area through the placement it
last drew the view at. The host sends the composition back the other way, as
input events of their own alongside the text it already delivers: an input
method turning on, the text being composed, the text it settled on, and its
turning off again.

Presence is block data like any other and carries no messages of its own. All
of it - the coarse "someone is looking at this block" entry as much as the
cursor - is published and read on the one block client the plugin owns, the
same client it reads the block through. The host publishes none of it, and no
other plugin reads it for a block it is not itself drawing. That single client
is what makes presence add up: a block's entries all carry one identity per
person, so a reader can join a colour to a cursor, and the server never returns
to a client the presence that client posted, so a user is never among the
people their own editor shows them.

An instance may also speak to the other clients in its block's live session
directly: it sends a payload on a channel (a uuid naming what the payload
means) to one client, by the id presence carries, or to all of them, and the
host relays it sealed with the block's key, as it relays the session's own
traffic, so the server cannot read it. What other clients send it arrives in
order, as peer messages naming the client and the channel. Nothing is kept:
a message reaches the clients in the session when it is sent, and an instance
that was not open, or a client that has left, never hears it. The terminal
block is built on this; its host machine shows a presence entry of its own,
and the bytes between it and every viewer travel as peer messages.

The only thing the host owes an instance about presence is whether its block is
currently being viewed. The framework answers that by publishing and clearing
the viewing entry itself, and passes it on so an instance knows when to start
and stop publishing a cursor; an instance that wants no cursor of its own
ignores it. The framework also draws the people viewing the block from the same
client, and asks the instance to reveal one of their cursors when the user picks
that person out, which only the instance can do because only it knows where in
its own content that cursor is - an in-process call on the app, not a message.

An editor instance may be asked to replace one of the blocks it references with
another - a copy the host made of a block being edited in two places at once -
which the host cannot do itself for a block type whose references live inside
its own content. The request names the block to replace and the one to put in
its place, and is answered exactly once with whether the instance made the
change. An instance that answers that it did not leaves the host to make the
replacement through the block's own structural edit.

An editor instance may ask the host to open another block in its own tab.
The host decides whether to honour the request; it is not answered, and a
request for a block the host cannot open is discarded. The request may name
the block the instance opened it from, which is what tells the host that the
block was reached through a reference held by that container rather than on
its own.

The instance that draws the window rather than a block - the one the host
opens on the workspace's own interface block - is where those requests end up:
the host hands it every block it is asked to open, its own included, as a
request of its own carrying the same block, block type and container, along
with the block the instance that asked was opened on, absent when the host
itself asked. Nothing answers it; the instance decides where the block goes,
since it owns the tabs, and the block that asked is what tells it whether the
block was reached from something already on the screen.

The host tells every open instance which block the user is looking at - the
block of the tab that has focus, absent while none has - along with the chain
of containers that block was opened through, innermost first. It is sent when
it changes and whenever an instance opens, which is what lets an editor that
lists the organization's blocks mark where the user is. The instance that owns
the tabs reports it the other way, with the same call: only it knows which
tab has focus, and the host passes what it says on to every other instance.

An editor instance may tell the host that it has started dragging one of the
blocks it draws. The host carries the drag from there, reporting it to
whatever the pointer moves over, so a block dragged out of one editor may be
dropped into another. The drag ends with the pointer, and is not answered.

An editor instance may ask the host to act on one of the blocks it draws: to
share it, to replace an occurrence of it inside a named container
with a copy of its own, to take it out of where it is listed, to move it from
there into another block, or to place it under one. These are structural edits
an instance cannot make for itself, and the ask to share, which the
host passes to the instance that draws the window to show its own dialog for;
each names where the block is listed - the root, the blocks with
no parent, or a container - and whether it is listed there as a reference
rather than as a child, since that decides whether the block's own parent
moves with it. None of them is answered.

The same request carries three more the instance that owns the window needs,
for blocks it is showing rather than drawing: to rebuild a block's dynamic
artifact or to unlink it from the source it was generated from; to hold a block to no more access than a named
level, which is what shows a block as an account with less access would see
it; to reveal one client's presence cursor, which only the editor of that
block can scroll to; and to close a block's editor, which is what a tab
closing means to the host. None of them is answered either.

An instance may ask the host to tell it about the dynamic artifacts of the
blocks it names, which the host answers with, and repeats whenever any of it
changes: for each of those blocks, the block it was generated from, the block
type that generated it, what its settings currently produce, why they cannot
be read, and whether it is being rebuilt. Only the plugin that made an
artifact can describe it, so the host keeps that instance open for as long as
a block is watched and answers on its behalf. Each watched block is a watch of
its own; dropping the last watch on a block closes the instance describing it. That instance
also shows a block's artifact settings: an instance places a child showing
them, as a dialog, whose status says whether the copy being edited differs
from the stored settings and summarises what it would produce. The settings
instance draws the dialog and its Apply and Cancel, and answers with
`close-dialog`; the editor showing it commits that child to store the copy
when the answer is `accepted`, and closes it either way.

The instance that draws the window may ask the host who can open a block,
answered once with the people the account may share it with: whoever already
reaches it and the account's contacts in the organization, never the rest of
its members - their account, email, name, whether they administer the
organization, whether they made the block, the access granted to them on it if
any, the access they end up with and whether they still wait for its key - or
with why it could not be listed. It may grant one of them a level of access to
a block, which reaches the host before any request the instance makes in the
same turn, so a listing asked for after a grant includes it, and it may ask for
a contact link carrying a grant of the block, answered with the link or why it
could not be made. A grant is not answered; the instance lists the block again
to see it, and lists it again when the account's contacts, a value the host
publishes to that instance, change.

An editor instance may ask the host to choose a file for it, which only the
host can do on every platform the app runs on. The request carries the filter
the picker offers and is answered exactly once, with the file the host read,
the picker the user closed, or why the file could not be read. Requests are
identified per instance and may be outstanding together.

An instance the host resizes - one embedded where the user drags its corner,
which the manifest's resize mode allows - is told the size it was given, in
its own points. Only an editor whose block records its size answers by
writing it; the rest ignore it and keep reporting the size they want.

Files the user drags onto a view reach the instance the way a dragged block
does: while they are only hovering the instance is told where they are and
nothing else, and on the drop it is told the same position along with each
file's name and the bytes the host read. Only the host can read a file on
every platform, so a plugin never sees a path.

Only the host can read the clipboard on any platform. Each frame an editor
instance reports the kinds of paste its focus takes (text, images), in the
order it prefers them, and the host pastes the first of those the clipboard
holds into the focused instance as a paste event of its own rather than as
ordinary typed text: text, split into pieces under the text limit, or an
image's name and bytes. The host pastes on the paste shortcut it sees itself,
and when an instance asks it to paste, as a menu's Paste does.

An editor instance may ask the host to play the audio in one of its blocks,
which only the host can do: a plugin has no sound device on any platform, and
an audio file is far larger than one call should carry. The request names the
block rather than its bytes, which the host already holds through its own
client. The host answers with what its player is doing - whether it is
playing, where it is, how long the audio is once it knows, and why it could
not play - and repeats that answer whenever any of it changes.

An editor instance may ask the host to fetch a URL for it, which is the only
network a plugin has: a plugin runs with no sockets of its own, and its block
client already reaches the server through the host's connection. The request
carries the URL and is answered exactly once, with the body the host read or
with why it could not be read. Requests are identified per instance and may be
outstanding together. The host refuses a URL that is not https, or whose host
name is not one the plugin's manifest names, and reports the refusal as an
ordinary failure rather than as a protocol error.

An editor instance may read the files its plugin was staged with, which are
read-only data the build lays out beside the app rather than anything an
organization holds: a plugin has no file system of its own on any platform. It
may ask for the list of those files, answered with every path relative to its
data, and for one file by that path, answered with its bytes or with why they
could not be read. A plugin reaches only its own data; a path that is not a
plain relative path is refused as an ordinary failure.

An editor instance may ask the host for web views, each a window of the
operating system's own laid over the app rather than anything a plugin could
draw. The instance names each view it opens with an identifier of its own,
opens it at a URL, asks it to navigate, reload or hand the keyboard back to
the app, and shows it by placing it as a child, like a block; a view no child
places is hidden. The host maps the child's rectangle through the placement it
last drew the instance's screen at, hides the view while the instance is not
being drawn, and closes it with the instance. It reports back
what the page does - a navigation started or finished, a history entry pushed
or replaced, a title, a history traversal the page asked for, a window it
wanted to open, the address the view is now at - and why anything it was asked
for could not be done. A platform with no embedded browser answers the first
request with that failure and nothing else.

An editor instance may ask the host to hold the pointer still and hide it, so
an editor that looks around a scene reads motion rather than a position. Only
the host can do that: the window is its own. The request says whether the
instance wants the cursor held, the host holds it while any instance does and
lets it go as soon as none does, and an instance that closes lets go with it.
While it is held the host reports raw pointer motion to the focused instance
alongside the events it already sends, since there is no position to report.

An editor instance may ask the host to put text on the clipboard, which only
the host can reach: the plugin's own ui context has no window under it. The
text an instance copied while drawing is sent as it is drawn and the host puts
it on the clipboard for it; a paste comes back the other way as a paste
event.

The host describes its registered block types once per plugin runtime, before
the first editor instance is opened, so an editor can name and illustrate
blocks it only holds a reference to. Each description carries the block type,
its display name, the codepoint of the host's icon font, and which of the
host's structural edits - adding, deleting and replacing a child - blocks of
that type accept.

The host reports a block dragged over an editor instance's view in that
screen's own logical coordinates, once per frame while it hovers, once more
when it is let go, and reports that the drag has moved off the instance again.
An instance answers with whether it would take the block, which only decides
the cursor the host shows; a drop is delivered whether or not it was accepted.

An instance draws its views: each a surface of its own, named by a `view-id`
unique among the instance's views, and placed by exactly one placer, which
the view's `view-placement` names. View 0 is the instance's main view, the
frame it is edited in. The host asks for every view another placer places
with a screen request (`screen-request.view` and `placement`) and sizes it:
`parent`, placed by the instance's placer as part of where the block is (its
main view, its preview, its artifact settings, the `parent-view`s a manifest
lists in `views`); `dock`, placed by the root dock as the body of one of the
instance's own dock tabs, bars or dialogs, or of an empty pane (a
`dock-tab-path` names that body by its view id); and `host`, a window the
instance opened or a further display. A view can also place itself against
its parent view, as a menu hangs from its button: an anchored view
(`anchored`), which the host never asks for. There is one way for an instance
to create a view, whatever places it: in the frame that creates it, the
plugin lays the view out, paints it as a screen it numbers itself from 2^63,
lists that screen in its layout before the commit, and reports it in the
commit, so a created view costs no frame; how it is placed decides what the
host does with it: a window (`window-request.open`) the host places and from
then on asks for, an anchored view (`frame-report.anchored`) the plugin keeps
placing itself. The host numbers the
views it asks for other than the main view from the top (the preview 2^63,
the artifact settings 2^63 + 1, the view on further display `n` 2^63 + 1 + n);
the instance numbers the views it makes itself, never 0 and below 2^63.
Screens, frame reports, child placements, input, child statuses and
`view-stopped` all name a view. A host that shows a desktop on several
displays shows the main view on its first display and a view placed by the
host on each further one, numbered from 1 in the order it lists them. A
screen request may say the host shows the windows the plugin opens from it
as windows of its own (`windows`). The plugin then opens each window itself,
as it creates any view: in the frame that opens
it, the plugin lays the window out at the size it wants and paints it as a
screen of its own, which it numbers from 2^63, lists in its layout and
reports in the same commit as the `window-request` that opens it (`open`,
naming the view the window will be, that screen, the view it opens from and
a placement: at the pointer, at a point of a view, or where a `host-place` the
host gave says). A window is always a window of its own; what points back to
where it came from (a menu, a picker, a popover) is an anchored view. The host shows the window from that
screen in the frame that commits it, where the placement says, moved back
inside its bounds, and from then on asks for the view with that same screen,
placed by the host, whose size, scale and `host-place` are the host's answer,
whose focus arrives as input, and whose going away closes it; the plugin
draws the screen as it opened it until the host's screens name it or it
closes the window. The other requests, `begin-move` and `begin-resize`, which
hand the pointer to the host until the press is let go, `raise`, `resize`
and `close`, each name the window's view. block-app says `windows` for every
main view it shows and, where it opens no windows of its own, draws each one
over its own window at the point asked for, moved back inside it, or shrunk
to the room below that point when there is enough. Without `windows` the plugin draws its
windows inside the screen. The main view is the whole rectangle an editor is
edited in, and an instance has at most one; the plugin lays its own toolbar
row, sidebars and content out inside it, in one pass of one context. The host
says, per frame, whether the instance owns the chrome bands this frame, where
its content goes inside the main view, and the trail of editors it sits
under. An instance told it owns no chrome draws its content alone: that is an
editor embedded in another one, a creation dialog, or an instance presenting.
An instance told it owns the chrome and given no content rectangle fills its
own content band, which is what a tab of its own means.

Docks cross plugins the way windows do: one root dock, the outermost, holds
the only dock state and draws every tab bar, splitter and drag; a dock nested
in it only declares its own tree and draws its tab bodies (`dock-message`,
`dock-update` in `dock.wit`, records at the end of `types.wit`). An instance
that places a child in the body of one of its dock's tabs names that tab
(`child-placement.docks`); the host then requests the child's frame with
`screen-request.docked`, and the child's dock declares itself
(`dock-message.declare`, a `dock-declaration`: a `dock-tree` flattened as
`dock-item`s with depths, as menus are, and each tab's `dock-tab`, with a key
that names it for good) and draws no chrome. The host splices each
declaration into its placer's in place of that tab, as a pinned group
(`dock-group.child`) whose tabs it names through the child
(`dock-tab-path.via`) and whose keys it prefixes with the tab's key and the
child's block id (`dock_prefix`, `DockDeclaration::splice`), up to the root,
the instance whose own frame is not docked: only the root hears its docked
children's declarations (`dock-update.child`) and requests
(`child-request`, whose request already names its tabs through every child in
`owner`), and hosts them (`dock-message.host`, `close`). The host
splits what the root hosts back down (`DockHosted::own`, `DockHosted::child`),
so each docked instance hears only its own tabs (`dock-update.hosted`, a
spliced child's group again the tab it was placed in) and its own closes. The
root places a descendant's tab body as a child showing `child-content.dock-tab`
with the full path, and the host shows the owner's view of that body there,
placed by the dock,
only for tabs the owner's `dock-hosted.visible` lists, and for the bodies a
declaration names that are no tabs: a `dock-dialog`'s (which the root presents
as it presents its own dialogs, over the dialog's parent, closing it with a
`close` of the dialog's `tab`), a pane's `empty` view, which the root shows
while the pane holds no tab, and a `dock-bar`'s. A bar belongs to a tab
(`owner`) and shows while it does, over or beside an edge of its body or
popped out (`dock-bar-placement`); the root gives its body the `size` the owner
declares, measured as the owner last laid it out along the bar's edge, and
the layout says where each bar is (`dock-layout.bars`). Its owner draws the
grip too: placing it is a `place-bar` request, and dragging it a run of
`move-bar` requests with the pointer in the bar body's own points (`move`,
then `drop` or `cancel`), which the root carries as a drag of its own, showing
where the bar would land. A `bare` pane shows its one tab with no tab bar. A
layout also names where each tab moved out of the declaration's group went
(`dock-layout.outside`: a pane of the root's by key and index, or a window of
its own), so a `restore` puts them back; each docked instance saves its own
layout, since the root saves only the tab it hosts in place of the group. The host keeps a docked
child open while its placer places it or, once the child declares a dock, still
declares the tab it was placed in (for the root, from the declaration it hears until it lets go with a `host`
of `none`), requesting its frame at its last size without drawing it; a tab closes
when its owner stops declaring it. Every view has its own menu: an instance
sends `shell.menu` for a view, and a pick from it, wherever it was shown,
arrives as its `menu-pick` of that view. The host gives the root each
spliced tab's menu as that `dock-tab`'s `menu` (a declaring instance leaves it
empty), whatever level the tab's owner is at; the root shows it on the tab's
bar and sends a pick as `dock-message.pick` with the tab's path, which the
host hands the owner as a `menu-pick` of the tab's view, so the owner knows
which tab asked (and presents what the command opens, such as its command
palette, over that tab's body, not in its frame).
A `menu-pick` carries `anchor`, in host coordinates, what the menu belongs to:
the tab that shows the view, or the rect a parent placed the editor at when
it has no tab; a dialog the command opens points at it, and `none` (nothing
of it visible, as in a fullscreen tab) opens it at the top centre of the
frame. The root fills it in as `dock-child-pick.anchor` and a placer as
`child-menu-pick`'s `anchor`, each from its own placement of what it showed
(`child-placement.anchor`: the tab a child fills, or the child's own rect),
and the host passes it on unchanged; a pick the host routes to a child that
took the frame over carries `none`, so that child points at its own rect.
The same rect reaches a view as `frame-spec.anchor`, in the view's points, so
a dialog opened from a shortcut or the palette points at the tab too.
A bar action of a docked child whose dock the host spliced (its frame is not
shown), such as asking for its details, reaches its placer as a `child-bar` as
if from its frame.
Popping a tab out keeps the root as the placer of its body: the root holds
the only dock state, so it draws the popped-out window's chrome (its tab
strip, the grab bar that moves it, the drop targets that dock it again, its
tabs' menus and locks) as a window it creates as it creates any view, in the
frame that pops the tab out, and places the tab's body in it
(`child-content.dock-tab`), so the owner's view stays placed by the dock
whichever plugin owns it, keeping every feature it has, and only its size
changes. An instance popping out a view of its own that no dock places opens
a window for it the same way.

An editor's features belong to the view that holds its content, not to view
0. That view is the one whose frame report gives `content` (insets); a view
holding none of the editor's content, such as a bar of its tools or a window
of its own, reports `none`, and so does a docked main view whose content is in
a dock tab. The host hands the camera to the view holding the content
(`view-changed`, `change-view`), shows a child that takes the frame over (an
`active` or `live` child placed without `own-frame`) over that view, and says
whether the instance's chrome is drawn through that view's `frame` (a dock
view holding the content is requested with one; the chrome of a docked main
view counts for nothing). A child placed in a dock tab's body with
`own-frame` joins the dock (`child-placement.docks`); a child placed anywhere
else, such as a card on a canvas, draws its dock itself. A main view that is
not docked still declares its dock, and a child that takes over the frame of
a docked instance's tab keeps its stage there, drawn over that tab's content,
while its bars join that tab's pane in place of the instance's own bars,
which the root stops showing until the child lets the frame go
(`DockDeclaration::splice_bars`); the host says so with a `hosted` of only its
bars (`DockHosted::frame_child`), and the child draws its bars itself again
when that turns `none`. A child of such a child that takes its frame over
replaces its bars the same way. A docked editor's
tab view's menu is its frame menu, or the menu of the child that took its
frame over, after the tab's own items; the menu a placer reads as
`child-status.menu` is the menu of the view holding the child's content. A root showing a joined group of one bare pane in place of the tab
that hosts it shows that group as the tab: the tab's title, icon, lock and
close, and the bare pane's tab's menu followed by the hosting tab's.

A frame surface is transparent wherever its owner does not paint, so whatever
the host draws beneath shows through there. The plugin publishes, with every
frame that laid it out again, the content area it laid out, as insets from
the frame's edges (or `none` where the view holds no content), and the rectangles it painted and the rectangles its
floating layers took, in the frame's own logical coordinates. The host works
out the view it hands a pan-and-zoom editor against the content area those
insets leave of the frame's current size, so a frame resized since the
editor last drew is handed a view for its new size in the same frame. It
delivers no pointer input over the parts of the frame the instance did not paint, and composites the frames of
one tab bottom-up: every frame's painted area first, then every frame's
floating rectangles, so a menu opened by an editor underneath still falls over
the frame above it. The two passes are cut into disjoint rectangles, so no
pixel is blended twice.

Anchored views carry what points back to where it came from (menus and their
submenus, popovers, tooltips, the selection toolbar, small dialogs opened from
a control) and sheets attached to an edge out of the view that opened them,
with no round trip; the host draws each as a layer above every view. A screen
request may carry a layer space (`screen-request.layers`): `to-host`, which
maps the view's points to host coordinates (the app window's logical points)
at the camera the request lays it out against, and `bounds`, the rectangle in
host coordinates where views anchored to it may go: the app window for a
trusted plugin, the plugin's own box for a confined one. A child's bounds
never exceed its parent's, and a screen requested without a layer space
anchors no views. An anchored view is a screen like any other, which the
plugin creates as it creates any view: in the frame that opens it the plugin
places it, anchored to a rectangle in its parent's coordinates or attached to
an edge of the bounds (`view-anchor`), lays it out against the bounds
(flipping, sliding or shrinking it to fit), numbers its screen from 2^63,
lists that screen in its layout with a surface it configured itself, paints
it, and commits its report with that frame. The report says how it hangs
(`frame-report.anchored`: its parent, the view the screen request was for or
another anchored view of the instance for a submenu, its anchor, and its
rectangle in host coordinates), and every report of it repeats that; the view
closes when the plugin's layout no longer lists its screen, and with it the
views hanging from it. Everything else is the screen's own, in its own points,
which are host points from its rectangle's top-left corner: what it painted,
where it takes input (`floating`) and the scrims it dims, its accessibility
description, which the host places where the view is drawn and leaves out of
its parent's, and its child placements (`child-placements.view` naming it),
which the host places in host coordinates, ordered against the view's pixels
by their `layer`, clipped to the view and its bounds and hidden by no
occluder; a child's own frame appears one frame after the view that holds it.
The plugin lays an anchored view out and draws it at full size in host
points, so a menu opened in a view shown at half size is as large as any
other menu, at the screen's raster divided by `to-host`'s scale, in pixels per
host point: the app window's own density, whatever the view's zoom, since the
screen's raster already counts it. The host draws anchored views after every
view, in the order they opened across every view of the window: each one's
scrims, then its `painted` rectangles from its surface; it draws nothing of
one outside `painted`, and the commit that opens one draws it in the same host
frame. When the view a chain hangs from moves, the host moves the chain
rigidly by the difference between the `to-host` it shows that view at and the
one the frame was laid out against, so a move alone need not send the plugin
a new `to-host`: the host may keep sending the origin it sent last while only
the origin changes, and sends where the view now is with the next input it
gives the view; a pan of the view's camera does nothing to them, and the
plugin repositions or closes them on its next layout. The host hit-tests
anchored views before any other view, over their `floating` rectangles, the
one opened last first, and input that lands on one reaches its own screen in
its own points; a plugin takes input for any screen it opened itself and
ignores input for one it has closed since. A press outside a chain of anchored views marked
`dismiss-on-outside-press` (a view and those hanging from it) closes the
chain: the instance hears the views in `input-batch.dismissed` of the screen
the chain hangs from, before that batch's events, and the press still reaches
whatever it landed on. A press on a menu closes only the submenus hanging
from it. A view marked `keyboard` gives the instance the keyboard while it is
open, and one marked `blocks-owner` keeps presses on the view the chain hangs
from from its content.

A popup that embeds another editor is an anchored view holding a child
(`child-placements.view` naming the anchored view): the instance that asks
lays the view out, draws its card and beak if it has one, and places the child
in it, whose own pixels appear one frame after the view. That is how an
instance asks for a block (the picker, below), renames one
(`child-content.rename`, run in the editor whose manifest accepts `rename`,
opened as `opening.rename`, which says it is done with `close-dialog`,
arriving as the child's `child-status.dialog`) and shows the app's menu
(`host-panel.app-menu`, which the host draws and reports done the same way,
its size as the child's intrinsic size). A press outside the view closes it
and removes the child, which closes the editor in it.
In the browser, where a plugin draws in a worker of its own, the page gives
that worker one canvas over the whole app on its first anchored view and keeps
it; the worker draws each anchored view's `painted` rectangles into it where
the page says, the page clips the canvas to the plugin's bounds, and the
canvas of the plugin that opened the newest one is on top.

A frame report may also carry press claims: a set of modifiers and a
rectangle. While every one of those modifiers is held, a pointer press inside
the rectangle belongs to the instance itself, not to the children placed
under it there: the host delivers that press, its drag and its release to the
instance, and none of it to the child (another editor, or a program's
window). A window manager written as a plugin claims the windows it places
this way, so a drag with the modifier held moves or resizes the window rather
than reaching the program inside it. The claims of an editor nested in another
yield to the claims of the editor around it.

An instance whose chrome is only reserved keeps the bands exactly where they
were and paints nothing in them, so selecting or leaving a child moves neither
its content nor the view it holds. The instance that draws the chrome also
draws the trail back out of it, and reports that the user asked to leave; the
host then hands the frame back to the parent.

An editor may be given a preview screen as well as the frame it is edited in.
The host maps that screen onto whatever quad it paints the block on, so the
editor fills it and lets the host place, rotate and fade it. An instance may
also report the shape of its block, which the host holds its preview to; it is
a request in the same sense as the embedded size below.

The host owns pan and zoom. Wherever an editor's content has a view of its
own, the host works out the rectangle that content goes in and tells the
instance, in the logical coordinates of its frame, along with the scale it is
showing that content at; the editor draws its content there and keeps no zoom
or offset of its own. The scale is what an editor with a world of its own
rather than a picture of a known size needs, since the rectangle alone says
how much of the world is on screen but not how large a unit of it is, and it
is what a zoom readout reports. An instance moves the view only by asking -
pan by a distance, zoom by a factor about a point in those same frame
coordinates, or fit - and the host answers by moving whatever viewport the
instance is being shown in, which may be a tab of its own or a canvas the
block sits in. An instance never told where its view is fills the content it
is given at a scale of one.

Wheel, pinch and middle- or space-drag over an instance's viewport belong to
the host, which reads them once for the outermost view they fall in: a gesture
over a block being edited inside a canvas moves the canvas, not both. An
editor leaves those gestures alone and turns only a gesture of its own into a
request.

An editor instance may report the size it would like to be given wherever the
host embeds it. It is a request, not a constraint: the host may embed the
instance at any size, and falls back to its own default until an instance
reports one.

An editor instance may embed other blocks' editors inside one of its screens.
A frame that changes them carries, for that screen, an ordered list of the
children it placed and the occluder rectangles declared between them, all in
the screen's own logical coordinates. A child
carries its own identifier, what it shows - a block and its block type, one
of the instance's web views, a panel the host draws itself, or a window of a
program the host runs - the rectangle
and clip it was placed at, the corner radius of the hole cut for it, whether
it composites below or above the instance's own pixels, and whether the host
should draw it as a preview, a passive editor, an active one, or a live one.
Occluders are ordered against the children: a child's interactive area is
its rectangle minus every occluder declared after it, which is the same
subtraction that decides what the host draws over it.

A child placed below is cut out of the instance's own surface, which is
cleared transparent, so the host composites the child's editor under the
instance and anything the instance draws after the child covers it again. A
child placed above composites against the instance's pixels instead and cannot
be drawn over. The host draws the children below a view, then the view,
then the children above it. A painting is never stretched: the host shows each
frame at the size it was painted, from the view's top-left corner, so while
a view is resizing ahead of the plugin the painting is cropped or falls
short at the right and bottom, and the children, occluders and input method
area that frame carried are placed in its own coordinates from that corner,
unscaled. Only a zoom the host renders a view at - a pixel density sent with
its screen - scales them, and then by the one sent at the frame's serial. A
child the placement says follows the view is moved from the view sent at that
serial to the one the host is showing, so a child keeps up with a pan the
plugin has not drawn yet.

A screen also carries its raster: the pixel density the host draws it at,
which its pixel size counts. It differs from its scale where the host shows
the screen larger or smaller than it lays it out at, as a panel in VR does. A
plugin lays out at the scale and paints at the raster, so a change of raster
alone moves nothing; until a painting at the new raster arrives, the host
shows the last one at the same size in points.

A child the user has selected, or one an instance declares as the whole of
what it shows, becomes a frame owner of its own: it is handed the same frame
as the instance that placed it, takes over every chrome band, and draws its
content at the rectangle the placement gave it. Its parent is told for that
frame that it owns no chrome, and keeps its content exactly where it was, so
selecting a child moves nothing. Handover is always a replacement: a child's
chrome is never nested inside its parent's, and the framework, not the plugin,
draws the trail back out.

A placement may instead declare that the child owns a frame of its own, which
is a frame laid out inside the rectangle the placement gave it rather than the
one the instance was given. The child draws its own chrome bands there and
begins a trail of its own, and the instance that placed it keeps its whole
frame, including every band, since nothing was handed over. That is what lets
an instance put several editors on the screen at once, each with its toolbar
and sidebars over its own part of it, which is how the workspace's own
interface draws a pane of files beside the block being edited. The two are
exclusive: a child that owns a frame of its own never takes the frame it was
placed in.

A child that owns a frame of its own may also be given the framework's top bar
over it, and the placement says which: the plain bar, or the one a phone
shows, which carries how many files the parent has open. The phone bar's back
button, title and file count do nothing inside the child: the instance reports
which of them was pressed, and the host hands that to the instance that placed
the child, which decides what going back, switching file or showing the file's
details means.

The panels the host draws itself - its debugging panels - are placed by the
instance that draws the window, never by the host: the host asks that
instance to show one, and draws it wherever a child places it.

The windows of the programs the host runs - on Linux it is a Wayland
compositor - are placed the same way. The host tells the instance that draws
the window every window it has whenever that changes: its title, its program's
id, the window it belongs to if it is a dialog, and the size it drew itself at.
The instance shows a window by placing it as a child, and the host draws the
program's surfaces there, tells the program the size it was placed at, and
gives it the input that lands on it. A window the instance does not place is
not drawn. The instance asks the host to close a window, and the window stays
listed until its program closes it.

A block pick may name a place for what it makes: the top level or a block. The
picker then asks for a name and lets the user choose the place as well, and
the block is created there under that name; the answer says so, and the plugin
only opens what it was given. A block chosen from the ones that already exist
is never placed by the picker, and the plugin places it as it would without a
place.

The host answers with a status per child: whether the block could be opened
at all, the size and shape its editor asks for, whether the pointer is over
it, whether it is being given input, whether its own editor is live or only
previewed until it is asked for, which of rotation, aspect ratio and pan-and-zoom
it allows, how it may be resized, and why it is unavailable when it is. A block
already open above the instance is refused, so an editor cannot be nested inside
itself. The host bounds how many children of one view it will run as editors and
falls back to preview rendering for the rest.

A placement may also carry the size the parent is giving the child, which the
host passes on as the size the child was resized to, and the rotation and opacity
to draw a preview child at, which is what lets an editor with a world of its own
lay a block out at an angle or fade it. Rotation and opacity are the host's to
apply to a picture it draws; a child it runs as an editor is drawn upright and
opaque whatever the placement says.

A child with a view of its own asks to pan, zoom or fit that view like any
instance does, and the host hands those requests to the instance that placed
it rather than acting on them: a canvas turns a block's request to fit into
fitting that block in the canvas's own view. An instance that ignores them
leaves the child's view where it was.

A child drawn as a preview is a picture the host draws rather than an editor
it runs: the instance declares the child as a preview and leaves a hole for
it, and the host composites the block's own preview into that hole, clipped
to the rectangle the child was placed at.

The host owns input routing. Nothing is delivered to a child until the
instance publishes it as active, so an instance keeps the clicks over its
passive children; while a child is active the host stops delivering pointer
input over the child's interactive area to the instance. Keyboard and text
input go to the innermost active editor, and the cursor comes from the
innermost editor the pointer is over. Promotion to active is the user's, so
the host takes it away again when the user presses Escape or clicks outside
the child. A live child is the instance's own content rather than something
the user promoted - the whole of what the view shows - so it is given input
from the moment it is placed and the host never takes that away.

An editor instance chooses a block by placing a picker as a child
(`child-content::picker`), which is what lets an instance acquire a child
without waiting for one to be dragged in. The child carries the filter - the
name of what is being chosen, the block types that may be chosen, empty for
any, the blocks never to offer, whether the picker opens on the templates the
host offers, and where to place what it makes - the location new blocks are
made in when the filter places none, and whether to lay the picker out for
touch, as sheets filling the child's rectangle, rather than as a menu and a
card sized to their content. The instance places the child in an anchored
view (above) hung from the control that asked, sized to the picker's last
reported intrinsic size, or one attached to the bottom edge of its bounds
filling them for touch; it frames the child with its card and beak while the
picker says it shows a card (`shell.show-card`), and a press outside the view
answers the pick with none. The host runs the child in
the editor whose manifest accepts `pick` (the workspace's `picker`), opened as
`opening::picker`; that editor reads the block graph for itself, so the
instance that asked never sees what the picker browses. The picker reports the
size its content wants as its intrinsic size and answers once with
`shell.picked`: the block chosen or created and whether it was an existing
block linked rather than a new one, none when the user cancelled, or why the
block could not be made. The answer reaches the instance that placed the child
as that child's `child-status.picked`, and the instance then removes the
child, which closes the picker. A creation dialog places a picker the same
way, which is how an editor whose block cannot be made until it references
another one gets that block before its own exists.

The picker makes new blocks by placing a child of its own that shows a
template being made, named by its editor and template from the catalog the
host sends with the block types. The child's status reports the creation's
progress - its options, whether they are ready, then the block it made or why
it failed - and the picker asks the host to make it, under a parent and with a
name, by committing the child.

An editor instance may ask the host to present it: to give it the whole window
and show nothing else of the app, which is what a slideshow or a video played
back full screen wants. The host owns the answer and reports the state it
settled on, since it may refuse, and takes presenting away again when the
instance is no longer the one on screen. A presenting instance is a frame owner
with no chrome bands, so it draws its content the same way either way; only the
size it is given changes.

An editor instance may report named duration and count measurements under a
named performance group, through the `be3:debug` extension's `performance`. Durations are measured by the plugin and sent as
nanoseconds; the host only collects and displays the reported values. Reports
are informational, require no response, and may arrive independently of a
rendered frame.

Neither peer polls the other: a plugin runs beside the host - a thread of its
own under wasmtime, a worker of its own in the browser - and either side may
send at any time.

Every host call that changes what a plugin lays out against - the screens,
which carries each screen's size, scale, raster and frame, and a view - is followed by
a configure carrying a serial, one higher each time, which says the host has
sent everything up to it. A plugin answers no configure: each frame it commits
names the last serial it had received when it laid out, so the host knows
which of the states it sent the frame was built from. The host keeps the
states it sent by serial and forgets each once a frame names a later one.

A frame is one commit of everything a plugin laid out together: the serial,
the size each view used, its frame report, its window requests - cursor,
input method area, intercepted keys and the rest - and its child placements
and occluders, along with how long it wants to wait before being drawn again
and the surfaces it presented. Each part is present only for a screen it
changed; one a frame leaves out still holds at the frame's serial, since a
plugin lays out again every instance whose host state changed. A frame may
move children without painting anything, and may paint without moving any.

The host decides when a plugin draws. A plugin paints only when the host asks
it for a frame, and never on its own: when a call changes what it shows, or
something inside it wakes it, it says a frame is needed and waits to be asked.
It says so once and not again until it has painted, so a plugin that hears
about a pan sixty times before it can draw once paints one frame carrying all
sixty rather than one frame each. Every published frame carries how long the
plugin wants to wait before it is drawn again, absent when it wants nothing
further, and the host holds that request until the delay comes due. The host
asks at the start of its own frame, along with the input it is routing, since
input it delivers is a change the plugin will want to show. Only one request is
outstanding at a time, so a plugin that draws more slowly than the host is
asked again only once the frame it owes has arrived, and a plugin animating
without any delay is drawn once per host frame rather than as fast as its
process can run. A request left unanswered for a second is asked again. A
plugin the host is not drawing is asked for nothing.

The host shows whatever the plugin has committed by the time it finishes
building its own frame. Which surface each view is drawn from is settled then
as well, against the layout that arrived with the frame being shown, so a
plugin that republishes its layout mid-frame is never drawn through the
placements of the one before it.

Every screen an instance is given is a surface of its own, the size the host
asked for, so a screen that grows, shrinks or goes away leaves every other
screen's surface as it was. Which surface a screen is drawn into is the layout
the plugin publishes: a screen keeps its surface for as long as it is shown,
and a surface whose screen is gone is released. A frame presents only the
surfaces whose screens changed, and the damage it reports names the screen
each rectangle falls on.

A plugin never learns where its pixels live. It asks its host for a render
target and draws into an opaque texture, which is the host's own under
wasmtime and the worker's offscreen canvas in the browser. Which surface each
screen is drawn into is the layout the plugin sends whenever its screens
change, and the host shows each screen's texture until another frame presents
it. A frame that presented anything counts its presents, which is how the host
matches the damage it reports to the textures it took in, even when it skipped
some of them. No pixels and no native
graphics resources cross the connection.

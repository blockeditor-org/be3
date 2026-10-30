# Vision

What BE3 is for, and why it has the parts it has. Where a direction is still
open, it says so. What each part of the repository is lives in
`guides/repository.md`.

## The problem and the bet

Every application writes files to a file system, and then reinvents, or goes
without, live collaboration, sharing, permissions, accounts, undo, history and
teams. BE3 is a bet on a new base for computers: every document is a block, and
every block gets those from the platform instead of from its application.

It is for the people who use Google Docs, Notion or Obsidian today. A person or
their company would pay for storage. If outside developers come, an extension
store could earn something, and if the desktop side gets good enough, hardware
could. None of that is decided.

## What it has to be

Two things, and neither is enough alone.

- **A strong core for extension.** What a block gets for free has to be good
  enough that building on BE3 beats building a standalone app, and the plugin
  API has to be safe and stable enough for strangers to build on.
- **Apps that win on their own.** A few apps have to be solid, or new, enough
  to attract people who don't care about the vision. An app is worth building
  first when it is clearly better than what people use today, or does something
  they can't do elsewhere, and shows off the core while doing it. The apps
  people need from day one (notes, sharing, a calendar) still have to be good.

## The core: layers a block can use

The platform offers layers, and each block uses as many as it needs. What goes
through a layer gets that layer's guarantees, and nothing else does.

1. **Identity, organization and sharing.** Accounts, workspaces, folders,
   names, members and permissions. Every block gets these, even one that is
   only a link.
2. **Connection setup.** Finding the other people in a block and introducing
   their clients to each other. A shared game world can stop here: BE3 sets up
   the join, and the game talks over a channel of its own.
3. **Storage and history.** Encrypted, content-addressed bytes, commits, and
   three-way merge of offline edits. The server stores objects it can't read.
4. **Live sync.** Today every block goes through a session owner that puts each
   operation in one order, so nothing has to be a CRDT. Once plugins can
   provide block types, a plugin should be able to bring its own instead: a
   CRDT that needs no true order, OT, or a game's own netcode that has no
   operations at all, while still using the layers below.
5. **The built-in model.** `be_model` documents get merging, undo and
   addressing of their parts without writing them. Most first-party editors use
   it.

Some blocks stop early:

- **A document kept by another service** is a link. The service keeps its own
  data, and BE3 gives the link a name, a place in a folder, and sharing (layer
  1).
- **A shared game world** uses layers 1 and 2.
- **A foreign file**, a format with no editor of its own, uses layer 3 as a
  blob. A blob can't be edited live, so it is locked instead, like cloud saves
  in games but a little better:
  - "Alex is editing this file. Edit anyway?"
  - "You are offline. Edit anyway?"
  - "This file was edited offline. Keep your version or theirs?", with a preview
    of both, so the choice isn't blind. Both versions stay in history, so a
    wrong choice can be undone. The stack already keeps the losing side of a
    conflicted merge as a commit (a replica's `Command::Stash`).

The guarantees follow the layers. "The server can't read it" and "every later
version can open it" hold only for what is stored in layer 3. A link or a game's
own channel keeps neither, and the app should be honest about that.

## Extension

Every editor is a plugin, so that eventually anyone can write one.

**Kinds.** Plugins are wasm only today. Native plugins existed once and were
dropped to save maintenance effort for now; they will come back. Web plugins
will be a third kind, supported on every platform: webtech is possibly the most
popular way to build a GUI there is. The protocol stays the same for all three,
and free of any one GUI framework. What differs is how messages travel and how
images are passed: wasm and native plugins hand over GPU textures, and a web
plugin passes none, the app just deals with it.

**Trust.** Plugins aren't properly sandboxed yet. The baseline to reach is that
the app can safely and automatically preview what a plugin draws, on your own
machine, even when a malicious person sent you the plugin. That is the least a
plugin can be trusted with. Installing one, so that it shows up in the add-block
menu and elsewhere, is a higher level of trust. Until the baseline holds, the
first release has to be more careful, and should not run a plugin without the
user confirming it.

**Licenses.** A core plugin may be GPL or AGPL, since a plugin is a separate
program, but the app must still build without it.

**Entry points.** The file tree is the only way into a block today. An editor
may want an entry point of its own, the way the Google Sheets app on Android
opens to your sheets and a button to make one. Not designed yet.

## Where it runs

- **Linux, Windows, macOS, the web and Android** run today.
- **iOS** is required before a release. It is deferred because it is hard to
  build for and there is no iPhone to test on. The app feels more like Android
  than iOS today, so after the basic port its behaviour needs close attention:
  back navigation, sheets, long press, text selection and scrolling.
- **The desktop.** If BE3 is the base of a computer, it has to be able to be the
  computer. As a login session (`BEUI_PLATFORM=drm block-app --desktop`), block-app is a Wayland compositor whose shell is
  a plugin (`guides/desktop.md`, `guides/desktop/linux.md`). On Linux, a foreign file's own program
  can then open inside a tab; elsewhere it opens in a separate app, and web and
  Android can't open it at all.
- **The PineNote** (`guides/pinenote.md`) is supported because it is a device we
  own. It shows the app owning a tablet's whole UI, at the cost of some e-paper
  specific work.
- **Embedded Linux**, maybe. A GUI framework with a DRM backend could compete
  with Slint, Qt Quick, LVGL and flutter-pi there, if its footprint and
  performance are good enough.

## Stances

- **No reach to strangers.** Every sharing or messaging relationship starts with
  a contact link given outside the app. That rules out spam, discovery and
  moderation problems, and is a stance on privacy and social features.
- **Being able to leave.** Export to plain files is part of trusting the app with
  your notes, and doubles as a second backup.
- **Frozen formats.** Nobody trusts an app with their notes if an update can
  break them (`guides/frozen_formats.md`).

## Decisions

**Why beui.** The app needs a GUI framework that works without compromises on
desktop, mobile and web, and isn't webtech. No big option does that:

- Flutter is the closest. It draws its own pixels everywhere, but on the web it
  has a large download, text input and selection that never feel native, and
  accessibility through a shadow DOM. It is also Dart.
- Compose Multiplatform, Qt, Avalonia and Uno are each strong somewhere and
  weak on the web or on mobile.
- The Rust options (Slint, Makepad, iced, egui, Dioxus, GPUI, Xilem) are small,
  so choosing one means owning it anyway and working around what's broken. If
  we have to own a framework to make it good, it may as well be ours.

The project's own needs point the same way: plugins are sandboxed guests that
render offscreen and hand over GPU textures, and the same UI is also a desktop
shell. Owning the framework means owning text shaping, IME, accessibility
(through AccessKit) and every platform's quirks. Drawn on the web, beui has Flutter's
problems, and Rust's wasm binaries stay large, though it already feels better
there than Flutter. The HTML core, below, is how it leaves them.

A compositor isn't usually written with a GUI framework: most composite client
buffers with simple GPU operations. GNOME Shell, KWin and COSMIC (smithay with
iced) do put one in the shell. What matters is that client windows stay
textures passed straight through, so the framework costs only what the shell
itself draws.

**beui on the web is the DOM.** Swapping leaf elements of a drawn page for real
DOM ones, as Flutter's platform views do, costs the same everywhere: a DOM
element can't sit between two layers beui draws, it must follow beui's clips
and scrolling without lagging a frame, and input and focus move between the
two. So beui has two cores under the same components (`guides/beui.md`,
Cores): the native one draws everything itself, and on the web the HTML one
makes each primitive a DOM element, so the browser lays out, focuses and
selects, pages render on the server and hydrate, and find in page and screen
readers see real content. beui keeps text editing its own in the first
version, the same code on both cores (the browser's EditContext and password
inputs may follow), and owns drawing, the escape hatch (`Drawing`) on both.
Plugins stay on canvas on the web for now, and later become iframes that could
run the HTML core themselves. block-app itself is still drawn on the web until
it moves to that core.

**Why a session owner instead of a CRDT.** A single order makes merging, undo
and the wire protocol simple, and lets an editor use plain data. Offline edits
merge three ways over the stored bytes, so a wholesale rewrite conflicts
visibly instead of interleaving. A block that is better as a CRDT can still be
one (layer 4).

## Editors

Each editor's own direction. Most are early, and existing ones aren't
necessarily the ones that should exist.

### Text

A core app for the first release, and the base of note keeping
(`plans/daily_driver.md`). It has to be fast on large files and feel native,
including text input on the web.

### Infinite canvas

A core app for the first release. Slides are canvases, so it is also the base
of presentations. It is headed to be a general 2D vector and raster image
editor with profiles for different tasks, like Affinity's: prototyping (like
Excalidraw, and first), vector, raster and interactivity. A profile is how you
view a canvas, not part of it. `plans/canvas.md` has the plan.

### Files, folders and the workspace

Where you find and organize everything. The file tree is today's only entry
point (see Extension). Search, history, trash and export (`plans/daily_driver.md`, phase 7) come next.
`be sync` keeps a folder and a directory on disk in step both ways
(`guides/sync.md`), so that block-app can one day hold its own source code while
buck2, editors, CI and agents still read real files.

### Version control

Version control is a powerful tool that only programmers get. The aim is to
make it good and easy enough for anyone, and good enough that this repository
could keep its source in block-app instead of git. Because collaboration is
already offline merges plus live edits, a third-party editor shouldn't need
special support to get it.

The model follows jj (Jujutsu) because its ideas are also the friendly ones.
There is nothing to stage and nothing "uncommitted": your edits are the current
change, kept as you make them, and saving it is giving it a description, after
which a new change starts on top. A change keeps its identity when it is
rewritten. A clash between two sets of edits is recorded in the change rather
than blocking you: you can keep working, save, and sort it out later. Every
step is in a log, and any of them can be undone, so nothing a person does in
it is a trap.

Revision history and version control are one concept, seen at two sizes. A
block's revision history is its line of changes: each autosave is a revision,
and naming one (a bookmark) marks a point worth keeping. A saved change is the
same thing across many blocks: a named snapshot of every block in a versioned
tree, made of the revisions those blocks went through since the last one. The
current change is simply the newest, unnamed end of that line. Still to do:
autosaves of a block inside a checkout should carry the current change's id
(commits already have the field), so a block's History panel can group its
revisions by the change they belong to and show its description, and a block
outside any repository keeps the plain line it has now.

### Database

Holds structured data, including the project's own tracker
(`guides/tracker.md`). **Open:** views should probably work like Notion's: rows
have properties, and a view filters and lays out the same rows as a table, a
board or a calendar. If any block could carry properties, the calendar would be
a view over blocks with a date and the map one over blocks with a place.

### Calendar

If BE3 is your computer, it needs a calendar. Possibly a view over dated
blocks, as above.

### Map

For notes tied to places. The current editor isn't that experience yet. It
might become a view over location metadata, as above.

### Chat

A group chat is a planned editor. It hasn't been designed. As a block it would
get members, permissions and history from the core, and links to the documents
it talks about.

### Presentation

A deck of slides, each one a canvas. `plans/presentation.md` lists ideas, such
as speaker notes and following the presenter live.

### Pixel art and video

Candidates for apps done well. Pixel art merges into the canvas's raster
profile, which might grow to be nearly as good as Aseprite. Light video editing
might be done well, or be dropped in favour of storing other tools' files as
foreign files.

### Image, audio and PDF

Viewers for the files people keep. They show that the stack streams large files
instead of loading them whole.

### E-book reader

Today, on the Linux desktop sessions (the PineNote first), a book opens in
KOReader, a Wayland program in a tab, which resumes where you left off with no
setup. It is the PineNote's default reader, and its typesetting (crengine) is
what people choose it for.

Later, a reader of our own on beui, for every platform. What it should do
better than KOReader is lay out only three pages, the previous, the current and
the next, never a whole chapter or book as crengine and MuPDF do. Pages are
anchored on the current page's top-left word: that word stays the top-left word
through a change of font, size or rotation, and it is what is stored as the
reading position (an EPUB CFI to the word, with a short quote and a fraction of
the book as fallbacks for a changed edition), so every device opens at the same
word. The next page is laid out forward from the first line that did not fit,
the previous one backward from the anchor, which can leave it short, as
Kindle's reader does. Without a whole-book layout there are no page numbers:
locations come from character counts, which also give the time left in a
chapter.

The book's XHTML and CSS are lowered into blocks of styled spans, close to what
a text block holds, so importing a book as text blocks is a cheap extra, and
the same paginated view could read any long text block. Fixed-layout EPUBs need
a browser engine and are left out. The PDF editor and this reader could share a
reader shell (page turns, positions, highlights, contents, search), not an
engine.

### Browser tab

Paused. A system web view never worked well, so the tab only says that the app
can't show web pages yet. A real browser means embedding Chromium, and would
make sense once block-app can be your whole desktop. Sharing a tab, so someone
else sees your browser live, is one possible direction.

### Games

A game is an application, so if the platform is good it has to run games. The
tabletop games (chess, checkers and others, as deterministic wasm modules) and
the logic game (a video game concept with some time put into it) are separate
projects that show what the plugin API can build once it is safe for third
parties and frozen. A shared game world that only uses BE3 to join (layers 1
and 2) is another shape a game can take.

### 3D scene

An experiment that might grow into a persistent VR world.

### Settings and the desktop shell

Settings are blocks too, so they sync and can be shared like any other. The
desktop shell is the editor that makes block-app a desktop.

### Terminal

A terminal block runs its shell on a machine its owner chose, reached through
the server, so any client can use a real terminal on a real machine, the web
and Android included (`guides/terminal.md`). The traffic is sealed like every
other session message. Sharing one hands out a shell as the host's user, so
sharing it needs warnings and a read-only default before a public release.
A terminal in the desktop session, running on the machine itself, is still
planned.

### Debug and example editors

The counter, triangle, pan-and-zoom, beui gallery, block-debug, checklist and
pixel ray tracer editors are examples, tests and experiments, not apps.

## Open questions

- How far the version control UI can go for someone who has never used git:
  whether branches need a plainer name, and how the change graph is drawn.
- Property-based views, as described under Database.
- How a plugin provides its own live sync, and what the app tells people about
  the guarantees such a block keeps.
- Entry points other than the file tree.

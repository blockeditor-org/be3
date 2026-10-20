# Vision

Why BE3 exists, what it is for, and why it has the parts it has. Little of this
was written down before, so where a direction is still open it says so.

## What it is

A new base for computers. Today every application writes files to a file system
and reinvents, or goes without, live collaboration, permissions, accounts,
undo, history, sharing and teams. In BE3 every document is a block, and every
editor gets those for free: live editing with other people, offline edits that
merge, encryption the server cannot read, history, and sharing.

The audience is people who use Google Docs, Notion or Obsidian today. A person
or their company would pay for storage. If the platform attracts outside
developers, an extension store could earn something too, and if the operating
system side gets good enough, hardware could. None of that is decided.

The first milestone is narrower: one person keeping their own notes in it
(`plans/daily_driver.md`). `plans/public_release.md` lists what other people
using it would need on top.

## What it has to do well

Hosting all your documents only works if a few things are done very well.

- **Text and the infinite canvas**, for the first release.
- **Maybe pixel art and light video editing.** Pixel art might grow to be
  nearly as good as Aseprite, or be dropped in favour of storing Aseprite's own
  documents. The video editor is in the same position.
- **A calendar.** If this is your computer, it needs one.
- **Everything else as a file.** A format with no editor of its own is kept as a
  blob (see Foreign files).

## The block stack

The `be-*` crates (`guides/the_new_block_stack.md`) are the "build it once"
layer. They replaced an older design whose single `Block` trait fused storage,
live edits, undo, references and children together, and whose failures set the
goals: state is bytes, live edits go through a session owner instead of a CRDT,
offline copies merge three ways, large files stream, and the server never sees
content. An editor describes its content with `be_model::Model` and gets merging
and undo without writing them.

Formats that hold people's notes are frozen (`guides/frozen_formats.md`):
nobody trusts an app with their notes if an update can break them. Export to
plain files is part of the same trust. Being able to leave is a reason to stay.

## Version control for everyone

Version control is a powerful tool that only programmers get. The aim is to make
it good and easy enough for anyone. It isn't there yet. Because collaboration is
already built on offline merges plus live edits, a third-party editor should not
need special support to get it.

**Open:** how it relates to per-file revision history (`plans/daily_driver.md`,
phase 7). From a user's point of view they should probably be one concept, but
that hasn't been designed.

## Foreign files

A file in a format BE3 has no editor for is stored as a blob. A blob can't be
edited live by two people, so it is locked, which works like cloud saves in
games, but a bit better:

- "Alex is editing this file. Edit anyway?"
- "You are offline. Edit anyway?"
- "This file was edited offline. Keep your version or theirs?", with a preview
  of both, so the choice isn't blind. Both versions stay in the revision
  history, so a wrong choice can be undone.

The stack already keeps the losing side of a conflicted merge as a commit
(`Peer::stash`). Its objects have to be held by the server before garbage
collection is turned on (`plans/daily_driver.md`, phase 1).

On Linux, where block-app is a Wayland compositor, the file's own program could
open inside a tab. Elsewhere it opens in a separate app. Web and Android can't
open it at all.

## Plugins

Every editor is a plugin, so that eventually anyone can write one. Today plugins
are wasm only. Native plugins existed once and were dropped to save maintenance
effort for now. They will come back, and web plugins will be a third kind,
supported on every platform. Webtech is possibly the most popular way to build a
GUI there is, so supporting it matters.

The protocol is meant to be the same for all three kinds and independent of any
GUI framework. What differs is how messages travel and how images are passed:
wasm and native plugins hand over GPU textures, and a web plugin passes no
images at all, the app just deals with it.

## beui

The app needs a GUI framework that works without compromises on desktop, mobile
and web, and isn't webtech. There is no big option that does that:

- Flutter is the closest. It draws its own pixels everywhere, but on the web it
  has a large download, text input and selection that never feel native, and
  accessibility through a shadow DOM. It is also Dart.
- Compose Multiplatform, Qt, Avalonia and Uno are each strong somewhere and
  weak on the web or on mobile.
- The Rust options (Slint, Makepad, iced, egui, Dioxus, GPUI, Xilem) are small,
  so choosing one means owning it anyway, working around what's broken. If we
  have to own a framework to make it good, it may as well be ours.

The project's own needs point the same way. Plugins are sandboxed wasm guests
that render offscreen and hand over GPU textures, which almost no framework can
do, and the same UI is also a desktop shell. Owning the framework means owning
text shaping, IME, accessibility (through AccessKit) and every platform's
quirks.

On the web, beui has the problems Flutter has, and Rust's wasm binaries stay
large. It already feels better there than Flutter, so this may not matter much.

**Open:** on the web, beui could keep its own layout but replace some leaf
elements with real DOM ones: text inputs first, then selection, links, video
and web plugins in iframes. That is what Flutter's platform views do, and it
costs the same things: a DOM element can't sit between two layers beui draws,
it must follow beui's clips and scrolling without lagging a frame, and input
and focus have to move between the two. Building on beui's unstyled components
keeps the option open, because an unstyled component can change its backend
without the styled one above it noticing. (`beui-renderer-dom` is something
else: it draws the display tree as DOM elements so fonts work on the web.)

## The desktop

Run with `--session`, block-app is the whole desktop: a Wayland compositor whose
shell is the linux-desktop plugin, with notifications, a tray, media keys, Wi-Fi
and a lock screen (`guides/linux_desktop.md`). If BE3 is to be the base of a
computer, it has to be able to be the computer.

This isn't how a compositor is usually written. Most composite client buffers
with simple GPU operations and have no GUI framework, though GNOME Shell,
KDE's KWin and COSMIC (smithay with iced) do put one in the shell. What matters
is that client windows stay textures passed straight through, so the framework
only costs what the shell itself draws. A GUI framework with a DRM backend could
also become a contender for GUIs on embedded Linux, against Slint, Qt Quick,
LVGL and flutter-pi, if its footprint and performance are good enough.

The PineNote (`guides/pinenote.md`) is supported because it's a device we own.
It shows the app owning a tablet's whole UI, at the cost of some e-paper
specific work.

## Platforms

Linux, Windows, macOS, the web and Android run today. iOS is required before a
release. It is deferred because it is hard to build for and there is no iPhone
to test on. The app feels more like Android than iOS today, so once the basic
port works its behaviour needs close attention: back navigation, sheets, long
press, text selection and scrolling.

## Sharing

Every sharing or messaging relationship starts with a contact link given outside
the app. The app has no way to reach strangers, which rules out spam,
discovery and moderation problems, and is a stance on privacy and social
features.

## What lives in this repository and why

Some of the repository is the product, some is tooling, and some is separate
projects that use the platform.

**The product.** The block stack, beui, block-app, the plugin API and SDK, and
these editors: text, folder, files, workspace, settings, canvas, presentation,
database, calendar, image, audio, PDF, video and pixel art. linux-desktop and
the `be-wayland` compositor are the desktop.

- **Database.** It also holds the project's own tracker (`guides/tracker.md`).
  **Open:** views should probably work the way Notion's do: rows have
  properties, and a view filters and lays out the same rows as a table, a board
  or a calendar. If any block could carry properties, the calendar would be a
  view over blocks with a date and the map a view over blocks with a location,
  one mechanism instead of several editors.
- **Map.** For notes tied to places. The current editor isn't that experience
  yet. It may become a view over location metadata, as above.

**Tooling.**

- **paint-review:** reviews the paintings tests make on a pull request.
- **be-launcher:** installs CI builds on Android and lists pull requests.
- **The tracker and `be`:** the project's own issues, kept in the app.
- **ghostty-vt:** the terminal core be-launcher shows build output with. It is
  kept building because it is the crate a terminal in the desktop session will
  use.
- **Debug and example editors:** counter, triangle, pan-and-zoom, the beui
  gallery, block-debug and checklist. Checklist is a demo, not meant as a real
  editor.

**Separate projects.** These are what could be built with the plugin API once
it is safe for third parties and frozen. They depend on beui and the plugin
system, so they live here to be updated in the same pull request as them. If
the platform is good, it has to run games too, since games are applications.

- **tabletop-games and the Games editor:** deterministic games as wasm modules.
- **logic-grid and logicgame:** a video game concept with some time put into it.
- **scene-3d:** an experiment that might grow into a persistent VR world.
- **pixel-ray-tracer:** an experiment.

**Unrelated.** cvl2 is a compiler for a language of its own that outputs
Minecraft datapacks. Nothing depends on it, and it is here because the
repository is a monorepo of Rust projects. It might one day write deterministic
games, compiled both to wasm and to plain English rules for playing them.

**Paused.** The browser tab. A system web view through wry never worked well,
so the app no longer opens one: the tab says the app can't show web pages yet.
A real browser will mean embedding Chromium. Sharing a tab with someone, who
then sees your browser live, is one possible direction.

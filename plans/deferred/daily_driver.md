# Daily driver

Depends on: plans/search.md, and plans/backlog.md item 1 (Push, don't poll).

A plan to get the host to the point where the user keeps their own notes in it.
Each item says what "done" looks like.

Real notes live in one remote workspace on a server running on a single VPS
(guides/hosting.md). Local-only workspaces (the embedded server) are permanent:
someone who never signs in keeps their work in one. Offline support on web waits
until before a public release (`plans/deferred/public_release.md`). Text and folder blocks
and everything under them are frozen (guides/frozen_formats.md); other block
types may keep breaking.

Start real use once Move to…, search and export below exist; offline open and the
status indicator already do.

## Data safety

- The server changes its in-memory object refcounts (`ObjectRefs`, in
  `be-server/src/blocks.rs`) before the transaction commits. A failed commit
  leaves them out of step with the database until a restart. Done: they change
  only once the commit succeeds.
- A conflicted merge keeps the unsaved side as a commit (`Peer::stash`), but the
  server does not count that commit's objects as held. Hold them before any
  collection runs (decisions/garbage_collection_waits_for_trash_and_history.md).
- A corrupt object on the server is only noticed when it is read, because
  `has` (`be-block-store/src/store.rs`) checks that the file exists, not that it
  is intact.
- Diverged sessions are reported only through the status error and the debug
  window.

## Formats

- **Streamed headers** (`TextHeader`, every blob header;
  `be-block/src/streamed.rs`) are a length and a bare postcard header with no
  version. Put a version before the header; the raw UTF-8 text after it stays.
- **A property whose stored value does not fit its shape** is loaded blank
  (`be-block-model/src/stored.rs`), and the stored value is lost on the next
  save. Keep the stored value and refuse writes to that property instead.

## Offline and flaky connections

The host is offline-first (guides/blocks/host.md, Disconnects, offline and stopping). Left:

- Commands that need the server themselves hold the worker's loop while it is
  silent, as do `peer.attach` after a reconnect, opening a block never seen
  before and merging a block a drain found diverged
  (`be-host-stack/src/worker.rs`). Done: each runs in a task of its own, as
  listings already do (`worker::Tasks`).
- `be_host_stack::retry_now` runs on `Lifecycle::NetworkChanged`, but only the
  web and Android send it. Send it from desktop network-change events too.

## Text editor at 120 fps on a large file

**Target:** a 10 MB / 100k-line markdown file. Typing, caret moves, clicks and
scrolling each stay within an 8 ms frame, and a focused, idle editor stops
waking the GPU. Each item has a work-count test in the style of
`a_long_text_area_only_builds_the_lines_in_view.rs`; profile again after each
one lands (guides/performance.md).

**Profile** (8.7 MB, 10,000 lines of 200 words, release build, Linux VM with
llvmpipe, `perf` with `BE3_PERF_MAP=1` for the app's wasm): holding an arrow key
4.5 ms per move; page down 28 ms per page (not re-measured since the first
profile); typing near the top 27 ms of app CPU per key: about 6.5 ms in the app
framework's frame, about 6 ms parsing the edited markdown window
(decisions/markdown_is_parsed_in_small_windows.md), about 2 ms splicing the
block content's flat `Vec<u8>`, and 1-2 ms each for the section scan,
`parse_block_urls` in `refresh_embeds`, and laying out the edited row. Outside
the app, per key, the be worker spends about 100 ms, mostly SHA-256 over the
whole document when it seals, and the host's main thread about 140 ms, mostly
llvmpipe.

1. **The text's bytes.** They are a flat `Vec<u8>`, spliced in O(n) at several
   layers. A rope or piece table is worth it once the whole-document copies
   below stop and measuring says the splice matters.
2. **Sections.** The collapsible section shapes are cached per document
   revision (`beui-text-editor-core/src/core.rs`), but a content change rescans
   the whole document, and the scan can reach far past an edit (a new heading
   ends the section above it). Make it incremental.
3. **Whole-document copies on every edit.** Left: the highlighter's own copy of
   the bytes; `parse_block_urls` over the whole text in `refresh_embeds`
   (`be-app-text/src/root/embeds.rs`), in `be-app-text/src/document.rs` and in
   `be-type-text`; the host's `session.bytes()` re-encode when it publishes.
4. **Sealing.** Every seal re-encodes, re-chunks, re-hashes and re-encrypts the
   whole document (`be-block-store/src/vault.rs`, `write`). Reuse the chunks
   before the first edited byte, since content-defined chunking resynchronises
   after the edit.
5. **Scrolling in apps.** It repaints the whole viewport, because the app path
   unions the moved region into its damage (`be-sdk-beui-platform/src/surface.rs`,
   `output.damaged()`). Carry the native presenter's GPU move copy across the
   app surface.
6. **Idle.**
   - Stop the caret blink (`CARET_BLINK`, `beui-core/src/base/text.rs`) after a
     few seconds without input, as most editors do.
   - Finish the "Push, don't poll" backlog item.
   - Done when a test confirms a focused, idle editor requests no frames.

Add a frame-time benchmark that types into a generated 10 MB markdown file
through the app host and prints p50 and p99 frame times, and run it in CI as a
work-count budget rather than a timing assertion. be-host-bench only pastes a
large text, and beui-host's `text-bench` types into a 3,000-line text area
without the app host.

## The notes app itself

Needed before daily use:

- **Move to… on touch and desktop.** The file tree's "Move to" submenu offers
  only top-level workspaces (`be-app-file-tree/src/root/menu.rs`). Add a folder
  picker sheet, and a Duplicate command (`be_host_stack::duplicate` exists).
- **Search** over names and content, in the file tree's filter field and a
  desktop quick-open (Ctrl+P / Ctrl+K): plans/search.md.
- **Trash.** Add "Delete forever" and "Empty trash". Restore already goes back
  to the original parent (`DELETED_FROM`).
- **Export and import.** Export a folder or the whole workspace to a directory
  of `.md` files, with links between notes rewritten as relative paths, and
  import a directory of `.md` files. Being able to leave is part of trusting
  the host with notes, and this is also a second, plaintext backup. Today
  "Export as" writes one block as its plain file or a block file, and import
  takes one picked file. `be sync` (guides/sync.md) already writes a folder as a
  directory and reads it back on native, with `be_block::plain` as the one
  mapping between a block and a file; build on those. What is left is the
  host's side (a directory picker, the web's download as a zip) and rewriting
  links.
- **Created and modified times** on `BlockInfo`, for sorting and recents.
- **Linking.** `[[` autocomplete to link a note while typing.

Nice to have later:

- a daily-note action;
- a heading level picker (the toolbar's Heading button always makes level 2);
- an Image button that picks a file (it inserts a placeholder `![alt text](url)`
  today, though the text editor can already create an image block and embed it);
- trimming the Add picker to the block types that are ready (it still offers
  its Debug group).

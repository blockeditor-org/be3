# What an editor asks of the host

An app has no network, no file system, no block store and no key of its own:
the host is its only peer. Everything it cannot do itself goes through the
`EditorHost` (`editor.host()`), whose methods document themselves; this guide
covers how those calls work as a whole and the parts that trip agents up.

## Asking and answering

A call the host answers (`save_file`, `export_block`, `read_plain_file`,
`fetch`, `list_data`, `read_data`, the revision calls, ...) is an async
function of the host-services interface
(`crates/be-sdk-api/wit/host-services.wit`), made for the editor's own
instance, and returns an `Asked<T>`. Run it with
`editor.then(asked, answered)` (`Creation::then` in a creation dialog): the
answer reaches `answered` only if the asking scope is still alive, and the
returned `Task`'s `cancel()` withdraws the call. Closing the editor cancels
everything it waits on, and the host drops the work (an unsent fetch, an
export not yet made).

A call that can fail answers `be_sdk_api::Failure`: `denied` (including the
host refusing the call outright), `unavailable`, or `failed` with a message;
its `Display` is the text to show.

In a test, `BeuiTest::take_requests()` returns the calls as
`be_sdk_test::HostCall`s, each answered with `answer(reply, value)`.

Notes on particular calls:

- `fetch(url)` is the app's only network, on every platform, and only to host
  names in the manifest's `network`.
- Files are picked with beui's own picker (guides/beui/components.md, Inputs and
  forms), which
  inside an app forwards to the host's; `be_sdk_beui::FileChooser` adds the
  import step. `save_file` saves through the platform's dialog or as a
  browser download.
- `read_plain_file(block)` is a block's plain-file form from its type module
  (guides/type_modules.md); renders such as PNG or PDF are the editor's own and go
  through `save_file`.
- `drag()` is a block the host drags over the screen; the editor draws the
  feedback and answers `accept_drag`. A drag inside the editor's own document
  never reaches the host: that is beui's `Draggable` and `DropTarget`
  (guides/beui/components.md, Drag and drop).
- `present(on)` asks for the whole window; the host may take it away (Escape,
  another instance on screen), so read `presenting()` rather than remembering
  what you asked.
- `occlude(rect)` tells the host the editor draws over children placed so
  far there.
- Web views can be placed (`SubregionContent::WebView`) but no host shows one
  yet: every open fails, and the editor shows that in its place.
- A revision preview is a temporary, read-only block id: the host isolates it
  from live updates and rejects edits from it and its nested editors. Release
  it with `release_revision`.
- The shell-only calls (share dialog, access lists, show and dialog requests,
  `close_editor`, ...) serve the instance that draws the host's window; no
  other editor has any use for them.

## The graph

The graph (which blocks exist, their names, parents, references and access) is
the host's mirror of the server's. `editor.watch_blocks(query)` is a memo of
the `BlockInfo`s for a `BlockQuery` (`Roots`, `Shared`, `Children`,
`References`, `Backrefs`, ...). Nothing lists every block, so a browser walks
a folder at a time from `Roots` and `Shared`.

`editor.blocks()` changes it: create, set parent, name, define metadata.
Each is a `GraphCommand` the host applies to its mirror at once, checks
against the account's access, and hands to its peer. Because the peer is the
host's, a block an instance just created still reaches the server after the
instance closes.

## Picking a block

`editor.pick_block(filter, anchor, picked)` acquires a child without waiting
for one to be dragged in. The SDK places a picker child
(`SubregionContent::Picker`) in an anchored view
(`crates/be-sdk-beui/src/popover.rs`, `ChildPopover`); the host runs it in the
editor that accepts `pick` (`be-app-workspace/src/picker.rs`). After a touch
or on a phone the view sits on the bottom edge and the picker draws itself as
a sheet. The view takes the keyboard as it opens, so typing reaches the
picker at once. `picked` gets the block, or nothing when cancelled; a new pick
replaces the one before. `Creation::pick_block` does the same in a creation
dialog, which is how the game editor gets its game module before its own block
exists. Tests: `BeuiTest::placed_picker()` and `answer_picker(pick)`.

`editor.rename_block(block, anchor)` and `editor.show_host_menu(anchor)` are
the same kind of popup, holding `SubregionContent::Rename` (run in the
workspace's `rename.rs`) or `SubregionContent::Host(HostPanel::Menu)` (drawn
by be-host). The frame menu's Rename anchors at `Editor::frame_anchor`.

## Watches and host calls

`editor.watch::<W>()` is a memo of a watch resource of the WIT world (a
resource named `*-watch` with `constructor()` and `next`), its `Default` until
the host answers; `editor.offered::<W>()` says whether the host has answered
at all. A watch is opened on first read and answered at once, on every change
and after an app restart. Each watch has its own revision, so one changing
wakes no other.

The core watches are in `wit/account.wit`; some (contacts, program windows)
are answered only to the shell (`be-host-runtime/src/published.rs`,
`shell_only`). The desktop's watches and calls are the `be3:desktop`
extension (guides/desktop.md), usable only by an app whose manifest names it.
A host without an extension answers its watches unsupported, which an editor
sees as never offered.

A host call is a function of the same interface made inside `editor.act`:
`editor.act(move || desktop::power::request(action).detach())`.

On the host, `be_host_base::watches::publish::<W>(&value)` sets a value,
`watched::<W>()` says whether anyone watches (the host starts PulseAudio only
then) and `take_calls(be_sdk_api::requested::power)` takes what instances
asked. `be-host-runtime/src/wire.rs` refuses an extension the manifest does
not grant or the platform cannot back. Adding one is a WIT resource or
function, a `publish` or `take_calls` on the host, and `watch` or `act` in the
editor; nothing else lists them. Tests use `BeuiTest::set_watched::<W>`,
`host_calls` and `take_host_calls`.

## Extending the protocol

If an editor needs something the host has, add it to the `be3:app` world in
`crates/be-sdk-api/wit` (`PROTOCOL.md` there lists the interfaces; how WIT is
written and travels is in guides/type_modules.md, Operations and WIT). An app's
request goes in the import that fits (`host`, `blocks`, `shell`,
`host-services`); a host request to an app goes in `app.wit`, as a method of
the `editor` resource when it concerns one instance. Every function is an
`async func`: one the app only tells the host returns nothing and is
`.detach()`ed; a followed value is a resource whose `next` the host answers
with each change.

- App side: exports are served in `crates/be-sdk/src/wire.rs` as `Delivery`s
  the runtime takes in order; imports are called directly under the editor's
  resource. A value delivered outside a runtime step (a watch's `next`) has to
  wake the session's waker, or nothing asks for a frame while a driven host's
  clock is stopped.
- Host side: `crates/be-host-runtime/src/wire.rs` takes the generated `Call`s.
  Answer every call where it is handled; a dropped `Reply` answers
  `Unanswered`. Block ids a call carries go through the checkout's scope
  (`calls::BlockIds`).

Anything both the host and apps draw (labels, painting helpers) goes in
`crates/be-sdk-ui`.

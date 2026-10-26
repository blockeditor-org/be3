# The terminal block

A terminal block is a shell that runs on a machine someone chose, the host, and
that every client the block is shared with sees and types into, on any platform,
the web and Android included. Its first use is running programs that only run
in a real terminal on a real machine, such as the `claude` command line tool,
from inside block-app.

## The pieces

- `terminal_types` (`crates/editors/terminal/types`, the terminal plugin's
  type module): the block's document (`Terminal`, below), the `TerminalHost`
  presence a host shows (its machine and user), and `TerminalMessage`, what
  viewers send the host. Its line helpers (`TerminalLine::spans`, `push`,
  `join`, `truncate`, `logical_lines`) are what both sides use to read and build
  lines.
- `be TARGET terminal-host [-- PROGRAM ARGUMENTS...]` (`crates/be-cli`,
  `terminal_host.rs` and `terminal_host/`): the host. It signs in as an agent,
  opens the block's replica in the type module (`replica::Session`, below),
  shows `TerminalHost`, runs the shell (the user's default one, or PROGRAM) in a
  PTY, keeps an emulator of it, and writes what the emulator shows into the
  document.
- `be_cli::replica::Session<S, C>`: one block's replica as `be` holds it, the
  same `be_types_host::Replicas` the app's peer runs, attached as the host's own
  editor (`HOST_EDITOR`) through an `EditorMirror<C>`. `content` is the mirror's
  content and `edit` operates on it; `show` and `send` are
  `FromEditor::ShowPresence` and `SendToPeers`, `presence` and `take_messages`
  what came back as `ToEditor::Presence` and `PeerMessages`; `is_owner`,
  `client` and `state` read the replica's status and lease, and `claim` asks the
  replica to claim the lease (`Event::Claim`). `next` waits for the server, the
  module or a deadline without losing anything when dropped, and `handle` acts
  on it. It needs a tokio `LocalSet`, where the replica's IO runs.
- `crates/terminal-emulator`: that emulator (`Emulator`): ghostty-vt fed the
  PTY's bytes, read back as `TerminalLine`s (the rows that scrolled off and the
  shown rows, `Shown`), and the key, paste and wheel encoders. The host and
  be-launcher's build output use it; viewers never do.
- `crates/editors/terminal`: the editor. It renders the document, and draws the
  setup page while the block is empty and no host is connected.
- `crates/terminal-view`: the beui view (`TerminalView`, below), which draws
  lines and knows nothing of where they come from, and the mapping from beui key
  presses and text to `TerminalKey` (`key_event`, `text_event`, reported as
  `TerminalInput`). be-launcher's build output feeds it from its own local
  `Emulator`, keeping the rows that scroll off in memory.
- `crates/ghostty-vt`: the bindings. Besides the render state (`Renderer`) it
  reads rows anywhere on the screen through grid references
  (`Terminal::active_row`, `history_row`, each with its soft-wrap flag), follows
  the top of the screen with a tracked reference (`Anchor`), and encodes keys
  (`KeyEncoder`, set from the terminal's modes), wheel reports (`MouseEncoder`)
  and pastes (`Terminal::paste`). `struct_layouts_match_the_library` checks every
  bound struct against the library's own type description.

## The document is the transport

`Terminal` is not a frozen format. It holds:

- `host`: the client id of the host writing it, and `shell`: the current
  shell's id, whether it runs, and its exit status.
- `history`: logical lines that scrolled off the main screen, oldest first,
  at most `HISTORY_LINES` (10,000); the host appends and drops from the front.
- `screen`: the main screen's rows; `alternate` and `alternate_active`: the
  alternate screen's rows while a program such as vim or less uses it.
- `cursor` (row, cell column, `byte`: where the caret goes in that row's text,
  which may be past its end, meaning the row is padded with spaces; visible,
  style and blink), `size`, the default `colors` and `title`.

A line or row (`TerminalLine`) is its text, style runs (byte length,
foreground and background as RGB or none for the default, bold, italic, faint,
underline, strikethrough; bytes past the last run are the default style) and
`wrapped`: the row continues on the next one. Rows are list items, so a
changed row is one small edit. Rows drop their trailing blanks unless they wrap.

After every read of the PTY (and every queued read behind it), the host writes
the difference as ordinary edits through its `Session`, split under 256 KiB:

- It feeds the emulator 256 bytes at a time and after each piece reads the rows
  that scrolled past the `Anchor`, so a large read cannot scroll rows out of the
  emulator's own small scrollback (4,000 rows) before they are read.
- A row that scrolled off becomes a history line by moving the screen row's
  object into `history` (its id stays the same), or, when the last history
  line wraps, by extending that line and removing the row.
- A resize can pull rows back out of the emulator's scrollback; the host takes
  them off the end of `history` again.
- Then it compares every shown row with the document and sets what differs, and
  sets the cursor, size, colours, title, shell and alternate screen.

The replica seals it on its own schedule, `SEAL_INTERVAL` (750 ms) after its
first unsealed edit, as every owner does (guides/type_modules.md).

## Who writes

Ownership of the live session is the lock between hosts. A host writes only
while its replica owns the session. When another client that shows
`TerminalHost` owns the session it stands by; otherwise it calls `claim()` every
3 seconds until it is granted (a viewer that owns the session seals nothing, so
its 30 second lease lapses). The replica ignores a claim until it has caught up
after joining, so the host never numbers the session from zero. The viewer then
follows the host like any old owner (guides/type_modules.md). The host's
replica heartbeats only when it seals; nothing claims from an owner that shows
`TerminalHost`, so a quiet shell's lapsed lease is not taken while it is
connected.

When a host becomes the writer it sets `host`, and unless the document's
`shell` is the shell it is already running, it restores: the old screen's rows
move into history (trailing blank rows dropped), the alternate screen is
cleared, a faint `Restored session: <date>` line follows, and a new shell starts
at the document's size (80x24 if it has none). When the shell exits, `shell`
says so, and the next key or paste moves the screen into history and starts a
new one.

## The view

`TerminalView` takes `history` and `rows` as `Lines` (line ids with shared
`TerminalLine`s; equal when the ids and the `Rc`s are), the `cursor`, the
default `colors`, and reports `TerminalInput`, `TerminalWheel` and `Fit`.

- Every line is one `Text` of `Span`s (colour, background, bold, italic,
  underline, strikethrough; faint is the colour halfway to the background) in
  `TextStyle::CODE`, in one `VirtualList` keyed by line id, inside a
  `SelectableText`, inside a `Scroll` with `follow_end` and the padding inside
  it. A history line wraps with `Overflow::Wrap { at: WrapAt::Anywhere, .. }`
  at the view's width, so in a monospace font it breaks where the terminal did.
  Screen rows joined by `wrapped` (`logical_lines`) are one `Text` with
  `Overflow::Visible` and a `<Break/>` at each wrap point, keyed by the first
  row's id, so a selection across the wrap copies no newline. They are not
  `Overflow::Ellipsis`: that would cut each row on its own and draw an
  ellipsis the terminal's cells do not hold whenever the view is briefly
  narrower than the grid. The list's `text` gives unbuilt rows' text to copy
  and select-all.
- The cursor is the row's caret (`TextCaret`, a bar) at `cursor.byte`, the row
  padded with spaces when the byte is past its end.
- Fit: columns and rows from the view's size and the cell, whose width is the
  monospace advance (`galley.cursor_pos(Pos2::ZERO, 1).x`), not the rounded
  galley width.
- Keys: a focusable `Interactive` with `clipboard_keys` inside the selectable
  region (its press handler takes the focus back from the `Selectable`, so typed
  text reaches it and Ctrl+Shift+C bubbles to the selection). Ctrl+C, Ctrl+V and
  Ctrl+A go to the shell; Ctrl+Shift+C copies (`copy_key`, no select-all key;
  the context menu keeps Copy and Select All); Ctrl+Shift+V is not handled, so
  the platform pastes and the paste arrives through `pastes`/`on_paste`. Typing
  scrolls to the end (`ScrollApi::scroll_to_end`).
- With `wheel` set the rows sit in an `Interactive` whose `on_scroll` reports
  whole lines and the cell under the pointer instead of scrolling.

The editor passes the main screen's history and rows, or, while
`alternate_active`, only the alternate rows with `wheel` set (sent to the host
as `Wheel`). The top bar's Main screen toggle shows the main screen read-only
meanwhile. Input, `Resize` (when the host is found or the fit changes) and
`Wheel` go only to the client in `host` while it shows `TerminalHost`, and only
from a viewer that may edit. Without a host the document still renders, and the
top bar says no machine is running it, with a popover of the setup steps.

## Messages

Viewers send the client in `host`, while it shows `TerminalHost`, peer messages
on the block's live session (guides/the_new_block_stack.md, Peer messages), so
the server relays them sealed and cannot read the terminal. Host and app are
replicas of one session, so both must run the same build of the module that
holds `TerminalContent` (the session's tag, guides/type_modules.md): `be`
carries that build in its executable.

- `Resize(size)`: the viewer's view changed, or it found the host.
- `Key(TerminalKey)`: a key (beui's `Key`), its modifiers, the text it typed
  and whether it was pressed. Typed text arrives as `Key::Unidentified` with
  the text. The host encodes it with Ghostty's key encoder set from its own
  emulator, so application cursor keys and the Kitty keyboard flags a program
  asked for apply.
- `Paste(text)`: pasted through `ghostty_terminal_paste`, bracketed when the
  program asked.
- `Wheel(lines, cell)`: for the alternate screen; mouse reports when the program
  tracks the mouse, arrow keys in alternate scroll mode, otherwise nothing.

Only the host's emulator answers a program's queries (cursor position, device
attributes), so a program gets one answer however many people are watching.

The host connects over a websocket (`connect_live`), not the HTTP posts the
rest of `be` uses, because the server pushes the relayed messages. A network
whose proxy refuses websockets cannot run a host. A host that loses the server
keeps the shell running and connects again, waiting longer each time up to 30
seconds; it recognises its own shell in the document and carries on without
restoring.

A viewer that may only read the block renders the document but sends nothing.
The host does not check that yet, and anyone who can edit the block has a
shell as the host's user: BE-E1C7A1 is the warning and permissions that should
come before a public release.

## Tests

- `be-cli`, each a real host running `sh` for an agent and a viewer (a
  `Session` of its own) reading the document on a real server:
  `a_terminal_host_writes_its_shell_into_the_document_for_its_viewers`,
  `a_restarted_terminal_host_keeps_the_history_and_marks_the_restore`,
  `two_terminal_hosts_take_turns_and_the_second_stands_by` and
  `a_terminal_host_encodes_keys_for_the_mode_its_program_set`.
- `block-app`'s `a_terminal_host_answers_this_peer_through_the_blocks_session`:
  a real host against the app's own stack, read from the block's content.
- ghostty-vt's tests cover rows, history, the encoders and paste; terminal-types'
  cover the line helpers and the messages.
- The editor's tests hold a document and drive presence and peer messages
  through block-ui-test (`hold`, `set_peers`, `take_sent_to_peers`): input
  routing, copying across a wrap and select-all over unbuilt history, the
  alternate screen's wheel and peek, and paintings of each state, the setup
  page included.
- terminal-emulator's test reads rows that scrolled off the screen.

# The terminal block

A terminal block is a shell running on a machine someone chose, the host, that every client the
block is shared with sees and types into, on any platform. Its first use is running programs
that need a real terminal on a real machine, such as `claude`, from inside be-host.

## The pieces

- `crates/be-type-terminal` (`terminal_types`): the document (`Terminal`), the `TerminalHost`
  presence a host shows, `TerminalMessage` (what viewers send the host), and the line helpers
  both sides use.
- `be TARGET terminal-host [-- PROGRAM ARGS...]` (`crates/be-tool-cli`, `terminal_host.rs` and
  `terminal_host/`): the host. It signs in as an agent, opens the block's replica, shows
  `TerminalHost`, runs the shell in a PTY with an emulator, and writes what the emulator shows
  into the document.
- `be_tool_cli::replica::Session`: one block's replica as `be` holds it, the same
  `be_block_type_host::Replicas` be-host's peer runs, attached as the host's own editor. It needs
  a tokio `LocalSet`.
- `crates/be-tool-terminal-emulator`: the emulator (be-lib-ghostty-vt fed the PTY's bytes, read
  back as `TerminalLine`s) and the key, paste and wheel encoders. The host and be-launcher's
  build output use it; viewers never do.
- `crates/be-lib-ghostty-vt`: the Ghostty bindings. `struct_layouts_match_the_library` checks
  every bound struct against the library's own type description.
- `crates/beui-terminal-view`: `TerminalView`, which draws lines and maps beui keys to
  `TerminalKey`, knowing nothing of where lines come from.
- `crates/be-app-terminal`: the editor, and the setup page shown while the block is empty and no
  host is connected.

## The document is the transport

The host writes its emulator's state into `Terminal` as ordinary edits through its `Session`,
after every PTY read: rows that scrolled off move into `history`, then every shown row that
differs, the cursor, size, colours and title are set. The replica seals on its own schedule like
any owner (guides/type_modules.md). Viewers just render the document.

Who writes is decided by the live session's lease: a host writes only while its replica owns the
session, stands by while another `TerminalHost` owns it, and otherwise claims every few seconds
(`CLAIM_INTERVAL`). A host that becomes the writer and finds a shell in the document that isn't
its own restores: the old screen goes into history behind a faint `Restored session: <date>`
line and a new shell starts. After the shell exits, the next key starts another.

## Input

Viewers send `TerminalMessage`s to the client in `Terminal::host` as peer messages
(guides/blocks/apps.md, Peer messages), only while it shows `TerminalHost` and only from
a viewer that may edit. The host and be-host are replicas of one session, so both need the same
build of the type module (guides/type_modules.md); `be` carries that build in its executable.

The host connects with a websocket (`connect_live`), unlike the rest of `be`, because the server
pushes the relayed messages; a network whose proxy refuses websockets cannot run a host. A host
that loses the server keeps the shell and reconnects, backing off to 30 seconds.

The host does not check who sends input, so anyone who can edit the block has a shell as the
host's user; BE-E1C7A1 tracks the warning and permissions.

## The view

Every line is one `Text` in a `VirtualList` inside a `SelectableText` inside a following
`Scroll`. Screen rows joined by soft wraps are one `Text` with `Overflow::Visible` and a
`<Break/>` at each wrap, so a selection across the wrap copies no newline
(decisions/terminal_rows_overflow_visible.md). The cell width is the monospace advance, not the
rounded galley width, or the fitted column count drifts.

Ctrl+C, Ctrl+V and Ctrl+A go to the shell; Ctrl+Shift+C copies; Ctrl+Shift+V is left to the
platform, whose paste arrives through `on_paste`. While a program uses the alternate screen the
editor shows only its rows and sends the wheel to the host; the top bar's Main screen toggle
peeks at the main screen.

## Tests

- be-tool-cli: real hosts running `sh` against a real server, read by a viewer `Session`
  (`a_terminal_host_*`, `two_terminal_hosts_take_turns_and_the_second_stands_by`).
- be-host: `a_terminal_host_answers_this_peer_through_the_blocks_session`.
- be-app-terminal: drives presence and peer messages through be-sdk-test (`hold`, `set_peers`,
  `take_sent_to_peers`), with paintings of each state.
- be-lib-ghostty-vt, be-tool-terminal-emulator and be-type-terminal test their own parts.

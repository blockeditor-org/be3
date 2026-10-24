# The terminal block

A terminal block is a shell that runs on a machine someone chose, the host, and
that every client the block is shared with sees and types into, on any platform,
the web and Android included. Its first use is running programs that only run
in a real terminal on a real machine, such as the `claude` command line tool,
from inside block-app.

## The pieces

- `be_block::terminal`: the block (`TerminalContent`, which holds nothing),
  the `TerminalHost` presence a host shows (its machine and user), and
  `TerminalMessage`, everything host and viewers say to each other.
- `be TARGET terminal-host [-- PROGRAM ARGUMENTS...]` (`crates/be-cli`,
  `terminal_host.rs`): the host. It signs in as an agent, joins the block's live
  session, shows `TerminalHost`, and runs the shell (the user's default one, or
  PROGRAM) in a PTY when a viewer first attaches.
- `crates/editors/terminal`: the editor. It draws the setup page while no host
  is in the session and the terminal while one is, switching as the
  `TerminalHost` presence comes and goes.
- `crates/terminal-view`: a ghostty-vt emulator drawn with beui
  (`TerminalScreen`, `TerminalView`) and the key encoding (`key_bytes`). The
  editor and be-launcher's build output both use it; the owner decides what a
  fit to the view means (be-launcher resizes its own PTY, the editor asks the
  host).
- `ghostty_vt::Screen::repaint`: the bytes that draw a screen again from
  nothing, which is how a viewer is given what the shell shows.

## How it travels

Host and viewers speak peer messages on the block's live session
(guides/the_new_block_stack.md, Peer messages), so the server relays them
sealed and cannot read the terminal. A viewer sends the host
`Attach(size)` when it sees the host, `Resize(size)` when its view changes and
`Input(bytes)` for keys, text and pastes. The host answers an attach with a
`Snapshot` (the shell's id, the last output sequence, the PTY's size and a
repaint of the screen its own emulator keeps), broadcasts every read of the PTY
as `Output` with the next sequence, and `Exited` when the shell ends. A viewer
writes output only onto the snapshot it follows; one that sees a sequence it
skipped, or another shell's, attaches again. Only the host's emulator answers a
program's queries (cursor position, device attributes), so a program gets one
answer however many people are watching.

The host connects over a websocket (`connect_live`), not the HTTP posts the
rest of `be` uses, because the server pushes the relayed messages. A network
whose proxy refuses websockets cannot run a host.

## Several viewers, reconnects and exits

- Every viewer sees the one PTY. The latest `Attach` or `Resize` sets its
  size, and every viewer is sent a new snapshot at that size; a viewer whose
  view is larger or smaller draws the host's size anyway.
- The host keeps the PTY whatever the viewers do. A viewer that comes back is
  repainted the screen: what is shown, its colours and the cursor, not the
  scrollback or the terminal's modes (bracketed paste, application keys).
- A host that loses the server keeps the shell running, keeps its emulator up
  to date and connects again, waiting longer each time up to 30 seconds;
  viewers attach again when its presence returns.
- When the shell exits, the viewers are told, and the next attach or input
  starts a new shell with a new id.
- A viewer that may only read the block draws the terminal but sends no input
  or resize. The host does not check that yet, and anyone who can edit the
  block has a shell as the host's user: BE-E1C7A1 is the warning and
  permissions that should come before a public release.

## Tests

- `be-cli`'s `a_terminal_host_runs_a_shell_for_its_viewers_and_repaints_it_for_one_that_returns`:
  a real host running `sh` for an agent and a viewer on a real server.
- `block-app`'s `a_terminal_host_answers_this_peer_through_the_blocks_session`:
  a real host against the app's own stack, through the worker's peer messages.
- The editor's tests drive it with presence and peer messages through
  block-ui-test (`set_peers`, `receive_from_peer`, `take_sent_to_peers`), and
  paint the setup page and a running terminal.

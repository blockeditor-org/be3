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
- `crates/be-sdk-terminal`: `TerminalView`, which draws lines and maps beui keys to
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

A host on a machine trusted less than the person's devices (a boat.dev sandbox, below) signs in
as a block's agent (guides/tracker.md, A block's agent): an app makes one with
`create-block-agent`, hands the credentials to the machine, and after the machine resumes from a
snapshot calls `rotate-agent-token` and hands it the new token. Its new connection closes the one
the machine left open and takes over the session at once. The `a_terminal_host_*` tests run as
such an agent.

The host does not check who sends input, so anyone who can edit the block has a shell as the
host's user; BE-E1C7A1 tracks the warning and permissions.

## A terminal host in a boat.dev sandbox

The boat app (`crates/be-app-boat`, `src/root/terminals.rs`) connects a terminal to a running
sandbox (decisions/a_sandboxs_terminal_host_is_a_block_bound_agent.md):

1. Connect makes a terminal block in the viewer's Unfiled and, when they may edit the boat
   block, adds it to its content (`Boat::terminals`) as a reference. A viewer sees their own
   terminal for a sandbox, else the first referenced one they can open; anyone else connects
   their own. Besides its own block and those below it, the host lets an app make an agent for
   a block it made (`CREATED_BY`) only while that block sits directly in the account's own
   Unfiled, authored by the account, with nothing below it
   (`be_host_stack::made_by_app_in_unfiled`): anyone who may edit a block can set `CREATED_BY`,
   and it stays on a block wherever it moves.
2. `create-block-agent(terminal, "boat VM <id>")`; the credentials, recovery phrase included,
   go in the viewer's owner-only `BoatSettings::terminals`, keyed by terminal block, since `be`
   signs in only with the phrase and a rotation answers only a token. When they can't be kept
   (the settings haven't loaded) the agent and the block are removed again.
3. `uname -m`, then `terminal-host-binary(arch)`: a deployed build answers the URL and SHA-256 of
   its own `be` (`build-info.json`'s `terminal_hosts`, which `./scripts/deploy` writes into the
   downloads); a development build answers the bytes of a `be` built into
   `~/.cache/be3/terminal-host/ARCH` (`./scripts/buck build //crates/be-tool-cli:native
   --target-platforms root//buck/platforms:linux_ARCH --out ~/.cache/be3/terminal-host/ARCH`; the
   failure names it), which goes up through an https upload URL unless the sandbox has it already.
   Either way a command checks the SHA-256 before `be` can run, fetching nothing when a `be` with
   that hash is there and staging under a name of the terminal's own, so two installs in one sandbox
   never share a file. Each build's `be` and `build-info.json` go in
   `/home/user/.local/lib/be3/builds/BUILD`, and `blocks/BLOCK` links the build a terminal runs, so
   a terminal connected from another build never replaces another's `be`.
4. The units `be-terminal@.service` and `be-terminal@.path` and the credentials file (the agent's
   credentials with `block` and `boat_id`) are written with the files API under
   `/home/user/.cache/be3`, then one command installs the units, moves the credentials to
   `/run/be3/terminal-host-BLOCK.json` (mode 0600, owned by `user`), links the build (deleting
   builds no terminal links that nothing wrote to for an hour, so one another install fetched and
   hasn't linked yet stays), deletes the staged copy and starts the service. When that command
   doesn't run, another deletes the staged copy, and every check deletes one left behind. boat keeps
   every command and its output 90 days, so no secret is ever in a command line. `/run` is in
   memory, so no snapshot carries the credentials, and the `.path` unit starts the service when they
   appear.

The service runs `be BLOCK terminal-host` as `user` with the file as `BE3_AGENT_CREDENTIALS`, and
stops after five failed starts in ten minutes (a revoked token). The host writes its standing to
`BE3_TERMINAL_STATUS` (`/run/be3/terminal-host-BLOCK.status`): `connecting` and since when before
each try, `connected`, or `offline` and why, which it keeps retrying. `be` refuses to start when
`BOAT_ID` names another sandbox than the credentials' `boat_id`; it is passed through from the
service manager, which boat may not give it, and `be` starts when it is absent, so this guards only
a fork whose services see its new id. Behind a gateway the host fails to connect and says so in its
log; the person adds the key by hand, as `BE3_AUTHORIZATION=...` in
`/home/user/.config/be3/gateway`, which the service loads.

Every resume leaves the sandbox without credentials. Each time the editor sees a sandbox with one of
the viewer's terminals running, it asks the sandbox whether the credentials are there, the build
`blocks/BLOCK` links (`none` when that build's `be` is missing, so it is installed again), the
host's standing, `systemctl is-active` and the clock, and asks the host for its own build
(`terminal-host-build`). A host that is active and connected, has just started, or has been
connecting for under two minutes is left alone, whatever token or build another device would have
written, so a device with stale settings or an older build never breaks a working terminal.
Otherwise (a resume, a revoked token, a build the server refuses, a connect that hangs, a service
that gave up) the row says "Terminal offline, reconnecting…" while the app calls
`rotate-agent-token`, which revokes the old token, keeps the new credentials and writes them again,
with this build's `be` when the sandbox links another or none. Build ids carry no order, so an older
device may install its build over a newer one, but only for a terminal that wasn't working. A resume
from outside be-host waits until someone opens the boat block. An answer for a terminal disconnected
meanwhile is dropped, and Disconnect waits for a check in flight.

Disconnect stops the service while the sandbox runs and removes the agent; only once the agent is
gone (or the server no longer has it, `agent-not-found`) is the terminal forgotten, and the block
stays in Unfiled. A terminal whose sandbox an unfiltered list no longer shows offers Remove, which
does the same; a list with a name or label filter calls no terminal gone. Connect needs the key's
`exec` and `file.write` actions; boat.dev's API refusing a command or file (403) offers Reconnect.

Tests answer boat's calls with recorded JSON and the host calls with `be-sdk-test`'s
`HostCall`s (`connecting_a_terminal_installs_its_host_in_the_sandbox`,
`a_resumed_sandbox_gets_its_terminal_back_with_a_rotated_token`, ...); the host's own side is
the `a_terminal_host_*` tests. Nothing reaches boat.dev or a real sandbox.

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

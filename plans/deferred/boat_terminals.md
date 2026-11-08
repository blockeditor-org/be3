# boat.dev terminals

Depends on: plans/boat.md, plans/unfiled.md and
plans/deferred/block_agents.md.

Opening a terminal block into a boat VM from the boat app (plans/boat.md). Set
aside until the blockers below are answered; the design follows them.

## Blockers

- **`be` in the VM must match the server's build.** The server refuses a `be`
  of another build (guides/tracker.md) and the terminal host and its viewers
  need the same type-module build (guides/terminal.md). The app needs the
  release URL and SHA-256 of `be` for the server it talks to, from a host call
  or a data file; nothing provides it.
- **The gateway key.** Credentials handed to the VM need the
  `Be3-Gateway-Authorization` key when the server is behind a gateway
  (guides/hosting.md); no app has it.
- **Testing.** A boat VM can't reach a local dev be-server, and production
  needs the pinned build and the gateway key, so Connect can't be tested end
  to end on a development machine.
- **Files and secrets.** boat's files API writes only under `/home/user` and
  `/tmp` (400 `invalid_path` elsewhere). Commands run as `user` with
  passwordless sudo, and every command and its output are kept 90 days in
  boat's command history, so a secret must never appear in a command line:
  write the credentials with the files API to a file under `/home/user`, then
  `sudo install` it into `/run/be3/` and delete the original.
- **One service per terminal.** One fixed file and service per VM would let a
  second terminal displace the first; use a templated `be-terminal@BLOCK`
  unit and a file per block.
- **Adding the reference** to the boat block's content needs Edit on it, which
  a viewer may lack; such a viewer's terminal lives only in their Unfiled.

## Design

- **Connect** makes a terminal block in Unfiled (plans/unfiled.md) and adds it
  to the boat block's content as a reference. A viewer sees the first terminal
  for that VM they can open; anyone else makes their own. A terminal reaches
  someone else only if its owner shares it or moves it into a shared place.
- **The host runs inside the VM**: `be terminal-host` as a block-bound agent
  (plans/deferred/block_agents.md), so the terminal block works unchanged on every
  platform. Connecting:
  1. `create-block-agent(terminal, "boat VM <id>")`.
  2. Write the credentials, block id and sandbox id as in "Files and secrets"
     to `/run/be3/terminal-host-BLOCK.json` (mode 0600). `/run` is in memory,
     so no snapshot carries them.
  3. Through the commands API, download `be` and check its SHA-256, and
     install the `be-terminal@BLOCK` service, started by a `.path` unit when
     its file appears.
- **Every resume** leaves the VM without credentials. When the app sees the VM
  running again it calls `rotate-agent-token`, which revokes the old token,
  and writes the new credentials. Until then the row says "Terminal offline,
  reconnecting…". A resume from outside be-host waits until someone opens the
  boat block.
- **Forks**: the terminal host refuses to start when `BOAT_ID` is not the
  sandbox it was installed for.
- **Disconnect** removes the agent at once. A VM gone from the list offers to
  remove its terminal, which removes its agent too.
- Someone who can read the VM's files can take every other key on it as well,
  so these measures only keep the terminal's credential worth no more than the
  VM.

## Order of work

1. Settle the blockers, then plans/deferred/block_agents.md.
2. Connect, rotation and Disconnect, with paintings for connecting and
   terminal offline.
3. Decision: a VM's terminal host is a block-bound agent. Update
   guides/terminal.md.

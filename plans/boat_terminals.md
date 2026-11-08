# boat.dev terminals

Depends on: plans/boat.md, plans/unfiled.md and plans/block_agents.md.

Opening a terminal block into a boat VM from the boat app (plans/boat.md).
The design follows the constraints.

## Constraints

- **`be` in the VM must match the server's build.** The server refuses a `be`
  of another build (guides/tracker.md) and the terminal host and its viewers
  need the same type-module build (guides/terminal.md). A host call
  `terminal-host-binary(arch)` (`x86_64` or `arm64`) answers where to get the
  `be` of the host's own build:
  - a deployed build answers the download URL of `be` under the deployment's
    `data/BUILD/` and its SHA-256 (guides/hosting.md, "Deploying"; the build
    and its pins are in `build-info.json`). The app downloads it in the VM
    through the commands API and checks the hash.
  - a development build answers the bytes of a `be` built locally for that
    arch, which the app uploads to the VM through the files API. When none is
    built, the call fails naming the buck2 target to build, and Connect shows
    that message.
- **The gateway key.** Credentials handed to the VM carry no
  `Be3-Gateway-Authorization` key (guides/hosting.md): no app has it, and none
  is handed out. Behind a gateway, the terminal host fails to connect and says
  so in its log; the person adds the key by hand (ssh into the VM and set it
  in the credentials file the service reads). Decided by the user; Connect
  only says the terminal could not connect.
- **Testing** runs against a mock of boat's API, never the real one: a fake
  boat server in the test answers the sandbox, files and commands calls from
  recorded JSON, and the terminal host runs locally against a test be-server,
  so Connect, resume and Disconnect are tested end to end without a VM.
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
  (plans/block_agents.md), so the terminal block works unchanged on every
  platform. Connecting:
  1. `create-block-agent(terminal, "boat VM <id>")`.
  2. Write the credentials, block id and sandbox id as in "Files and secrets"
     to `/run/be3/terminal-host-BLOCK.json` (mode 0600). `/run` is in memory,
     so no snapshot carries them.
  3. Get `be` as `terminal-host-binary` says (download and check its SHA-256,
     or upload it), and install the `be-terminal@BLOCK` service, started by a
     `.path` unit when its file appears.
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

1. `terminal-host-binary`, for deployed and development builds, with tests
   for both answers and the missing-build message.
2. Connect, rotation and Disconnect against the mock boat API, with paintings
   for connecting and terminal offline. Done: the end-to-end test connects,
   types into the terminal, resumes with a rotated token and disconnects.
3. Decision: a VM's terminal host is a block-bound agent. Update
   guides/terminal.md.

# boat.dev terminals

Depends on: plans/boat.md. Unfiled has landed (guides/blocks/workspaces.md),
as have owner-only settings (guides/settings.md, Owner only) and blocks'
agents (guides/tracker.md, A block's agent).

Opening a terminal block into a boat VM from the boat app (plans/boat.md).
The design follows the constraints.

## Constraints

- **`be` in the VM must match the server's build.** The server refuses a `be`
  of another build (guides/tracker.md) and the terminal host and its viewers
  need the same type-module build (guides/terminal.md). A host call
  `terminal-host-binary(arch)` (`x86_64` or `arm64`) answers where to get the
  `be` of the host's own build:
  - a deployed build answers the download URL of `be` under the deployment's
    `data/{build}/be-{triple}/be` and its SHA-256 (guides/hosting.md,
    "Deploying"). The bundle's build metadata (`be_lib_build_id::Metadata`)
    holds only `id`, `program_id` and `default_server` today, and the pins are
    in `scripts/internal/pins`, which a running host can't read, so
    `./scripts/deploy` writes each arch's URL and SHA-256 into it. The app
    downloads `be` in the VM through the commands API and checks the hash.
  - a development build answers the bytes of a `be` built locally for that
    arch, which the app uploads to the VM through the files API. When none is
    built, the call fails naming the buck2 target to build, and Connect shows
    that message.
- **The gateway key.** Credentials handed to the VM carry no
  `Be3-Gateway-Authorization` key (guides/hosting.md): no app has it, and none
  is handed out. Behind a gateway, the terminal host fails to connect and says
  so in its log; the person adds the key by hand: ssh into the VM and write
  `BE3_AUTHORIZATION=...` to `/home/user/.config/be3/gateway`, which the
  `be-terminal@` unit loads as an `EnvironmentFile` (`be` reads the key from
  `BE3_AUTHORIZATION`, not the credentials file). It survives resumes and
  rotations; that VM snapshots carry it is accepted. Decided by the user;
  Connect only says the terminal could not connect.
- **Testing** never uses the real API. The mock is `be-sdk-test`'s `HostCall`
  doubles answering boat's calls from recorded JSON: an editor test checks
  that Connect sends the right files and commands calls, that a resume
  rewrites the credentials with a rotated token, and that Disconnect removes
  the agent. The terminal host's side is the existing `a_terminal_host_*`
  tests, which run as a block's agent (guides/tracker.md, A block's agent). No HTTP fake and no
  end-to-end run in a VM.
- **Boat key actions.** The sign-in page in plans/boat.md asks only for the
  list, start and stop actions. Connect also needs the commands and files
  actions (`exec` and the file write action; verify the names against boat's
  `openapi.yaml`), so the sign-in page asks for them once this lands, and a
  key without them shows Reconnect when Connect is pressed.
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

- **Connect** makes a terminal block in Unfiled (`block-location::unfiled`) and adds it
  to the boat block's content as a reference. A viewer sees the first terminal
  for that VM they can open; anyone else makes their own. A terminal reaches
  someone else only if its owner shares it or moves it into a shared place.
- **The host runs inside the VM**: `be terminal-host` as a block-bound agent
  (guides/tracker.md, A block's agent), so the terminal block works unchanged on every
  platform. Connecting:
  1. `create-block-agent(terminal, "boat VM <id>")`.
  2. Write the credentials, block id and sandbox id as in "Files and secrets"
     to `/run/be3/terminal-host-BLOCK.json` (mode 0600). `/run` is in memory,
     so no snapshot carries them.
  3. Get `be` as `terminal-host-binary` says (download and check its SHA-256,
     or upload it), and install the `be-terminal@BLOCK` service, started by a
     `.path` unit when its file appears.
- **The credentials** `create-block-agent` answers, recovery phrase included,
  are kept in the viewer's owner-only `BoatSettings` kind
  (guides/settings.md, Owner only), keyed by terminal block, since `be` won't sign in
  without the phrase and a rotation answers only a token.
- **Every resume** leaves the VM without credentials. When the app sees the VM
  running again it calls `rotate-agent-token`, which revokes the old token,
  and writes the full credentials file again with the new token. Until then the row says "Terminal offline,
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

1. `terminal-host-binary`, for deployed and development builds, including
   `./scripts/deploy` writing `be`'s URL and SHA-256 per arch into the build
   metadata. Done: tests for both answers and the missing-build message.
2. Connect, rotation and Disconnect against the `HostCall` doubles, with
   paintings for connecting and terminal offline. Done: the editor test
   above passes, and the stored credentials survive a reload of the app.
3. Decision: a VM's terminal host is a block-bound agent. Update
   guides/terminal.md.

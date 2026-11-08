# boat.dev

Depends on: plans/app_network.md (step 1), plans/owner_settings.md,
plans/unfiled.md and plans/block_agents.md.

An app for boat.dev cloud VMs ("sandboxes"): list your VMs, start and stop
them, and open a terminal block into one. It is the first block that mostly
shows online data and the first that needs an outside account. Not implemented
yet.

## What boat offers

From `https://boat.dev/llms.txt`, `https://boat.dev/openapi.yaml` and
`https://docs.boat.dev/`:

- **API**: `https://boat.dev/api/v1`, with `Authorization: Bearer boat_…`.
  `GET /sandboxes` (cursor pages) lists `id`, `name`, `state`, `type`, `ip`,
  `sshEndpoint`, `labels` and more; `GET /sandboxes/{id}`;
  `POST /sandboxes/{id}/resume`; `POST /sandboxes/{id}/stop`, an operation
  polled at `/sandboxes/{id}/stops/{stopId}`. `archived` is the API's name for
  stopped. `GET /me` says whose key it is.
- **Keys** are made in the dashboard's API Keys tab or with
  `boat api-key create`, each with actions (`sandbox.read`, `exec`,
  `file.write`, ...) and an expiry of at most 365 days. An organization key
  sees only that organization. There is no OAuth flow for third-party apps;
  the agent sign-in flow (a link and a six-digit code) is meant for agents.
- **No terminal endpoint.** A shell is SSH (`POST /sandboxes/{id}/sshkey`; the
  IP and host key change on every resume) or `POST /sandboxes/{id}/commands`,
  optionally detached. There is a files API.
- **No push to clients**; webhooks need a public endpoint.
- **CORS** allows only `http://localhost:1420`, so the web build can't call it.
- **Creating** takes `type` from a fixed enum in the schema, with `xlarge`
  refused below some plans, and no call lists what may be made.
- **Snapshots.** Stopping keeps the disk as a snapshot; saved snapshots can be
  shared with any boat account, and everything on the disk goes with them.
  Every sandbox has `BOAT_ID` in its environment.

## The design

### Crates

- `be-type-boat`: the block's content, and the `BoatSettings` kind.
- `be-app-boat`: the editor. The manifest has `network: "*"` and the agent host
  calls (plans/app_network.md, plans/block_agents.md).

### Content and online data

- **Content** holds only what is ours: which boat account or organization the
  block shows, optional filters (name prefix, label), and each sandbox id's
  terminal blocks.
- **The VM list is never content.** Each viewer fetches it with their own key,
  so boat's own access decides what they see, and sharing the block leaks no
  VM names or addresses. The app keeps the last list in a cache on this device
  only, opens with it at once and says how old it is, or that it is offline.
- **Refreshing**: poll `GET /sandboxes` about every 15 s while the editor is
  visible, faster while a start or stop runs.
- **Actions**: start (resume) and stop. Resuming bills, so it asks first. No
  create, since boat doesn't list what may be made.
- **On the web** the editor says the app is not available on the web.

### The key

- **Kept by the app** in `BoatSettings`, an Owner kind (plans/owner_settings.md)
  in the account's settings, so it is per organization and synced. be-host
  knows nothing of boat.
- **Signing in** is the editor's own page: a link to the API Keys tab
  (`open-url`), which account or organization to choose, the actions
  (`sandbox.read`, `sandbox.resume`, `sandbox.stop`, `exec`, `file.write`,
  `account.read`), a 365-day expiry, the equivalent `boat api-key create`
  command, and a field to paste the key, checked with `GET /me`.
- **Expiry** is the app's: it warns two weeks before, and a 401 or
  `api_key_expired` shows Reconnect.

### Terminals

- **Connect** makes a terminal block in Unfiled (plans/unfiled.md) and adds it
  to the boat block's content as a reference. A viewer sees the first terminal
  for that VM they can open; anyone else makes their own. A terminal reaches
  someone else only if its owner shares it or moves it into a shared place.
- **The host runs inside the VM**: `be terminal-host` as a block-bound agent
  (plans/block_agents.md), so the terminal block works unchanged on every
  platform. Connecting:
  1. `create-block-agent(terminal, "boat VM <id>")`.
  2. Through the files API, write the credentials, block id and sandbox id to
     `/run/be3/terminal-host.json` (mode 0600). `/run` is in memory, so no
     snapshot carries them.
  3. Through the commands API, download `be` and check its SHA-256, and
     install a systemd service started by a `.path` unit when that file
     appears.
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

### Tests

- Editor tests answer `fetch` with recorded boat JSON. Paintings: signed out,
  the list, an empty list, cached while offline, a stopped VM, starting,
  connecting, terminal offline, key expired, and the web.
- A host test with a fake boat service. CI never calls boat.

## Order of work

1. plans/app_network.md step 1, plans/unfiled.md, plans/owner_settings.md and
   plans/block_agents.md, in any order.
2. The type, the sign-in page and the list with start and stop.
3. Connect, rotation and Disconnect.
4. Decisions: live data from an outside service is not block content; an app
   owns its credentials for outside services; a VM's terminal host is a
   block-bound agent. Update guides/terminal.md.

## Not planned here

- Creating VMs.
- The agent sign-in flow for people; it may later give a be agent its own boat
  identity.
- Matching `be` in the VM to be-host's build: formats freeze before a release.

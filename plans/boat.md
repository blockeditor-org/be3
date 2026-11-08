# boat.dev

Depends on: plans/app_network.md (step 1) and plans/owner_settings.md.

An app for boat.dev cloud VMs ("sandboxes"): list your VMs and start and stop
them. Opening a terminal block into one is deferred
(plans/deferred/boat_terminals.md). It is the first block that mostly shows
online data and the first that needs an outside account. Not implemented yet.

## What boat offers

From `https://boat.dev/llms.txt`, `https://boat.dev/openapi.yaml` and
`https://docs.boat.dev/`:

- **API**: `https://boat.dev/api/v1`, with `Authorization: Bearer boat_…`.
  `GET /sandboxes` (cursor pages) lists `id`, `name`, `state`, `type`, `ip`,
  `sshEndpoint`, `labels` and more; `GET /sandboxes/{id}`;
  `POST /sandboxes/{id}/resume`; `POST /sandboxes/{id}/stop`, an operation
  polled at `/sandboxes/{id}/stops/{stopId}`; `DELETE /sandboxes/{id}`.
  `archived` is the API's name for stopped. `GET /me` says whose key it is,
  but not when it expires or which organization it belongs to.
- **Keys** are made in the dashboard's API Keys tab or with
  `boat api-key create`, each with actions (the spec lists `sandbox.read`,
  `exec`, `file.read`, `snapshot.read`, `environment.read`, `account.read`)
  and an expiry of at most 365 days. An organization key sees only that
  organization. A scoped key lists no keys, and `/api-keys/scoped` may answer
  503. There is no OAuth flow for third-party apps;
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
- `be-app-boat`: the editor. The manifest has `network: "*"`
  (plans/app_network.md).

### Content and online data

- **Content** holds only what is ours: which boat account or organization the
  block shows, and optional filters (name prefix, label).
- **The VM list is never content.** Each viewer fetches it with their own key,
  so boat's own access decides what they see, and sharing the block leaks no
  VM names or addresses. Apps have no storage of their own on a device
  (guides/apps/host_services.md), so the last list is kept in the viewer's
  view state (synced, but the viewer's own), and the editor opens with it at
  once and says how old it is, or that it is offline.
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
  (`open-url`), which account or organization to choose, the actions the list,
  start and stop need (named as the openapi spec names them; check it when
  building), a 365-day expiry, the equivalent `boat api-key create` command,
  a field to paste the key and one for the expiry the person chose, checked
  with `GET /me`.
- **Expiry** is the app's: `GET /me` doesn't report it, so the app warns two
  weeks before the expiry the person entered, and a 401 or `api_key_expired`
  shows Reconnect.

### Tests

- Editor tests answer `fetch` with recorded boat JSON. Paintings: signed out,
  the list, an empty list, cached while offline, a stopped VM, starting, key
  expired, and the web.
- CI never calls boat. A manual check may: the API key on the machine running
  the work is in `~/.config/be3/`. Use one VM of the smallest type with a short
  `ttlSeconds`, and `DELETE` it when done; never leave one running or stopped.

## Order of work

1. The type, the sign-in page and the list with start and stop. Done: the
   editor tests and paintings above.
2. Decisions: live data from an outside service is not block content; an app
   owns its credentials for outside services.

## Not planned here

- Creating VMs.
- The agent sign-in flow for people; it may later give a be agent its own boat
  identity.

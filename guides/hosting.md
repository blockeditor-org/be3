# Hosting be-server

The deployment command is `./scripts/deploy`. It updates an existing server;
it does not provision its service, reverse proxy, data directory or backup schedule.
Those are managed on the host, with no installation templates in this repository.

## Server layout

`BE3_DEPLOY_HOST` selects the SSH destination (`ubuntu@blocks.pfg.pw` by default).
`BE3_DEPLOY_DIR` selects the directory relative to the SSH user's home
(`be-server` by default). The deploy user must be able to write there.
`be-server` and `build-info.json` live together in that directory; the server
reads the adjacent metadata without `--build-info-file`.
`BE3_DEPLOY_UNITS` names the systemd units to stop and start (`be-server` by
default), using `sudo systemctl`. Their configuration must run that executable
and keep the persistent data outside the files deployment replaces. Deployment
leaves the host's service configuration and reverse proxy alone.

The server speaks plain WebSockets at `/api/be` and the same protocol over
HTTP posts at `/api/be/http` for clients behind a proxy that refuses WebSockets.
The host's reverse proxy supplies TLS at `api.be3.pfg.pw` and routes those API
paths and `/auth` to the server. The web app is served separately by Bunny at `be3.pfg.pw`.
The proxy must allow the web app's origin; native clients send no Origin header.
A gateway can accept `Be3-Gateway-Authorization` from native clients and a
host-only cookie from browsers, whose WebSocket API cannot set headers.

The server's `--data-dir` holds `metadata.sqlite` and `objects/`. On startup the
server adds missing schema tables, indexes and defaulted columns, and refuses
incompatible schemas. This directory and its backups are frozen formats
(guides/frozen_formats.md).

Password sign-ins are disabled unless the host starts the server with
`--allow-login`; registration requires `--allow-registration`. Devices already
signed in reconnect with their session tokens. To create an account directly,
stop the server and run its executable with the same `--data-dir` and
`--add-account EMAIL NAME ORGANIZATION`, passing the password on standard input.

## Deploying

`./scripts/deploy` builds be-server, `be` and the app for every platform in release in one buck2 command
(local-tools deploy, in crates/local-tools) and then, from that build:

- uploads each `be` bundle (Linux x86_64 and arm64) to the storage zone as
  `data/BUILD/be-TRIPLE/` (`BUNNY_STORAGE_KEY`), reads it back from
  `be3.pfg.pw` to check the hash, and pins it in `scripts/internal/common.sh`,
  which `./scripts/be` must match the server by (guides/tracker.md);
- uploads the app's downloads to `data/BUILD/app/` in the same zone, each read
  back the same way: `:native` with its plugins for Linux, macOS and Windows on
  x86_64 and arm64 and the web bundle, each zipped on a worker by
  `buck-tools archive` into a folder of its own name, and `:android-release`,
  signed on a worker with the release keystore under the id and label
  `--android-id` and `--android-label` give (`com.be3.block` and `Block`, so
  it installs beside dev's `com.be3.block.dev` and CI's `com.be3.block.ci`), with
  `data/BUILD/app/index.html` linking them. Once the server runs the build (not
  under `--skip-server`), it writes the same page to the unlinked
  `web/downloads.html` with `noindex, nofollow` robots metadata, without reading it back
  through the CDN, and purges it from the CDN using the required `BUNNY_API_KEY`. A
  download connects only to a server of its build. `--skip-downloads` leaves
  the downloads out of the build, and `--skip-android` only the APK, so that
  neither needs the keystore;
- uploads the unpacked web bundle and plugins to `data/BUILD/web/` and archives the
  server executable and its build ID to `data/BUILD/server/` in the same zone;
- copies only `be-server` and its `build-info.json` to the VPS over SSH, stops
  be-server, swaps the files and starts it again. Caddy stays running;
- after that succeeds, publishes `/index.html`, the matching app HTML with a
  `<base href="/data/BUILD/web/">` before its import map and scripts. The browser
  stays at `/` or `/index.html`, keeping query parameters and fragments, so refreshing
  gets the currently deployed build.
  Purges the web entry point using the required `BUNNY_API_KEY`. `--skip-server`
  uploads versioned files but leaves both live entry points unchanged.

Versioned directories use the Git tree hash that identifies the build. Existing
objects may be reused only when their SHA-256 matches; deployment refuses to
overwrite a versioned URL with different bytes.

All versioned artifacts are read back and SHA-256 checked before publishing
live pages. The web entry point must also return the isolation headers below;
deployment stops before swapping the server if they are missing. The CLI pin includes its mirror URL, so the existing pinned CLI
continues to use its existing location until a successful deployment writes
both the new build and `https://be3.pfg.pw/data`. Third-party tool mirrors remain
on `lfs.pfg.pw`; they are independent of BE3 deployments.

Configure these response headers in the Bunny pull zone before serving the web
app (they are CDN settings, not files that the uploader can set in storage):

| Paths | Headers / caching |
| --- | --- |
| `/data/*`, `/`, `/index.html` | `Cross-Origin-Opener-Policy: same-origin`, `Cross-Origin-Embedder-Policy: require-corp` |
| `/data/*` | CDN cache TTL one year; `Cache-Control: public, max-age=31536000, immutable` |
| `/`, `/index.html`, `/web/downloads.html` | Override CDN cache TTL to one hour; send `Cache-Control: no-cache` to browsers |

The mutable pages stay cached at the edge until deployment purges their exact URLs after uploading replacements. Configure the CDN TTL independently of the browser header, so browsers revalidate while Bunny can serve its cached copy. Deployment checks for the purge key before uploading or swapping the server and fails if a purge fails. Dry runs and `--skip-server` do not require the purge key.

Use a prefix rule for `/data/*` and exact-path rules for the mutable entry points, so every later build inherits the cache policy. Enable directory index resolution to `index.html`. Preserve uploaded content
types, especially `application/wasm` and JavaScript MIME types. Uploading an
HTML entry point last avoids incomplete releases; long-lived versioned paths
avoid mixing modules and plugins from different builds. The public web bundle
contains no gateway key. Visit `https://api.be3.pfg.pw/` to unlock the gateway;
after setting its host-only cookie it redirects to the web app. Keep the same
gateway secret when adding the new API hostname, and establish a new cookie
there; the old host's cookie does not move between hosts.

After the build, Rust loads the storage zone's password (`BUNNY_STORAGE_KEY`,
`be-app-storage-key`) and the account's API key for the purge (`BUNNY_API_KEY`,
`bunny-api-key`, required when publishing live pages), asking at a terminal
for missing keys and saving them in `~/.config/be3`. Before the single buck2
invocation, Bash prepares the release keystore (`android-release-keystore.base64`),
which it can make anew and copies to `buck/android/release-keystore.base64`
for the signing action. Bash also handles options that configure the build. Android installs an update only when it is signed
with the same key, so keep a copy of the keystore off the machine: a new one
means every device uninstalls the app once. Like CI's, it is an input of the
signing action, so it is in the build server's CAS.

It refuses a working tree that differs from its commit, so every deployed build
can be built again. Commit the pin it writes. `--dry-run` prints each step and
changes nothing; `./scripts/deploy --help` lists the rest, and the environment
that names the host, the directory there and the storage zone.

## Backups

Backups are scheduled on the host. `be-server backup` writes a backup and
`be-server verify` checks the data; `be-server --help` describes the options.

- `be-server backup-key > backup.key` creates the key. Keep it readable only by
  the backup process and save a copy off the VPS: a backup cannot be restored
  without it.
- Run `be-server backup --data-dir DATA --to bunny:ZONE --key-file backup.key`
  with `BUNNY_STORAGE_KEY` (the zone's password) and `BUNNY_STORAGE_ENDPOINT`
  (the region endpoint, such as `https://ny.storage.bunnycdn.com`).
- The zone's password can delete what it wrote, so a compromised VPS can delete
  its backups. Turn on replication to a second region and copy backups to a
  machine the VPS cannot reach.
- Notes are end-to-end encrypted. Reading restored notes still needs the
  account's recovery phrase or an unlocked device.

## Restoring

1. Stop be-server.
2. `be-server restore --from bunny:ZONE --key-file backup.key --data-dir NEW-DIR`
   restores the newest database and every object it references into an empty
   directory. `--snapshot NAME` picks an older one from `databases/`.
3. `be-server verify --data-dir NEW-DIR`, then point the host’s service at it and start it. Devices reconnect with the tokens
   they had.

## Never

- Never run `CollectDetached` or `PruneHistory` against the server. The app
  never sends them, and the backups never delete objects either.

## Proxy account credentials

`Authorization: Bearer <account token>` is reserved for be-server account
sessions. Native and browser clients can keep sending their account token in
protocol messages. A proxy can instead inject the header into WebSocket upgrades
or every HTTP POST; clients send `AuthenticateHeader` to use it. The server
validates the same session tokens and returns an empty token field for this
request, keeping the injected secret outside the client. HTTP sessions require
the same account header on every request.

The agent dialog's proxy checkbox omits the token from its JSON bundle and sets
`authentication` to `proxy`. The recovery phrase stays in the bundle for local
decryption. `BE3_AUTHORIZATION` and the native app's gateway field now send
`Be3-Gateway-Authorization`; the host’s gateway rules must use that header. Browsers use the gateway cookie because their WebSocket API
cannot set request headers.

Remote accounts are added through the server's `/auth` page. The app starts a
short-lived authorization request over its gateway-authorized connection and
opens `/auth?code=...`. The page uses same-origin form posts, checks Origin and
an expiring form nonce, and applies `--allow-login` and `--allow-registration`.
Approval notifies the requesting connection; only that connection can claim its
session token, once. Closing the connection cancels its pending request. The
browser is assumed to already have the API host's gateway cookie. No gateway
or account token is put in the URL. The URL and code stay visible in the app;
`--session` does not launch a browser automatically.

The browser transport uses WebSockets, whose handshake includes the API host's
cookies even when the page is on `be3.pfg.pw`. HTTP fetches added for API access
must explicitly include credentials; the proxy must allow the exact frontend
origin and credentialed responses, and allow preflights without a gateway
cookie. The cookie remains host-only on `api.be3.pfg.pw`.

`./scripts/deploy` includes `default_server` in every bundle's `build-info.json`:
`name`, `api_url`, `auth_url`, and `gateway_required`. It contains public
configuration only. Ordinary local and CI builds omit it and offer local or
custom servers. The deployment command validates that its bundles carry the
deployed identity and default server. Saved accounts retain their own URLs.

The embedded server alone enables local profiles. Their UI asks only for a
display name, and saved tokens reopen them from the account picker. Closing a
local account keeps it in the picker. Browser-origin requests cannot access
the embedded server's account protocol.

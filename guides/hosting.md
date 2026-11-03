# Hosting be-server

`./scripts/deploy` updates an existing server. It does not provision its
service, reverse proxy, data directory or backup schedule: those are managed on
the host, with no installation templates in this repository, and deployment
leaves them alone.

## Server layout

`BE3_DEPLOY_HOST` selects the SSH destination (`ubuntu@api.be3.pfg.pw` by default).
`BE3_DEPLOY_DIR` selects the directory relative to the SSH user's home
(`be-server` by default), which the deploy user must be able to write.
`be-server` and `build-info.json` live together in that directory; the server
reads the adjacent metadata without `--build-info-file`.
`BE3_DEPLOY_UNITS` names the systemd units to stop and start (`be-server` by
default), using `sudo systemctl`. Their configuration must run that executable
and keep the persistent data outside the files deployment replaces.

The server's `--data-dir` holds `metadata.sqlite` and `objects/`. On startup the
server adds missing schema tables, indexes and defaulted columns, and refuses
incompatible schemas. This directory and its backups are frozen formats
(guides/frozen_formats.md).

be-host does not send `CollectDetached` or `PruneHistory`, and the backups
never delete objects either; don't run either against the server.

## Routes, the proxy and the gateway

The server speaks plain WebSockets at `/api/be` and the same protocol over
HTTP posts at `/api/be/http` for clients behind a proxy that refuses WebSockets.
The host's reverse proxy (Caddy) supplies TLS at `api.be3.pfg.pw` and routes those API
paths, `/auth`, and `/auth/*` to the server, and `/build/*` to the build helper
on `127.0.0.1:9091` (guides/build_server.md). be-host's web build is served
separately by Bunny at `be3.pfg.pw`.

The gateway accepts `Be3-Gateway-Authorization` from native clients
(`BE3_AUTHORIZATION`, and native be-host's gateway field) and a host-only cookie
on `api.be3.pfg.pw` from browsers, whose WebSocket API cannot set headers; the
host's gateway rules must use that header. Visiting `https://api.be3.pfg.pw/`
unlocks the gateway: it sets the cookie and redirects to be-host's web build.
The public web bundle contains no gateway key.

The browser transport uses WebSockets, whose handshake includes the API host's
cookies even when the page is on `be3.pfg.pw`. The proxy must allow the web
build's exact origin and credentialed responses, and allow preflights without
a gateway cookie; native clients send no Origin header. HTTP fetches added for
API access must explicitly include credentials.

### Proxy account credentials

`Authorization: Bearer <account token>` is reserved for be-server account
sessions. Native and browser clients can send their account token in
protocol calls. A proxy can instead inject the header into WebSocket upgrades
or every HTTP POST; clients call `authenticate` with no token to use it. The server
validates the same session tokens and returns an empty token field for this
request, keeping the injected secret outside the client. HTTP sessions need
the account header on every request.

The agent dialog's proxy checkbox omits the token from its JSON bundle and sets
`authentication` to `proxy`. The recovery phrase stays in the bundle for local
decryption.

## Accounts and the portal

Remote accounts are added through the server's `/auth` portal, the only remote
password and registration interface; be-host's protocol accepts browser
authorization, session tokens and proxy headers. There is no command-line
account provisioning. Password sign-ins require `--allow-login`; verified
registration also requires `--allow-registration`. To bootstrap a server,
enable registration, create and verify an account through the portal, then
disable registration if appropriate.

A deployed server needs `--auth-origin https://api.be3.pfg.pw` in its service
configuration. It is the exact public origin used for form Origin checks, Secure
cookies and email links; request Host and forwarded headers cannot override it.
Localhost HTTP development infers its origin, and web launchers supply theirs.

### Adding a device

be-host begins a short-lived authorization request over its gateway-authorized
connection and opens `/auth?code=...`; a login session opens no browser by
itself, and be-host keeps the URL and code visible even when a browser launch
appears to succeed. The user signs in or registers and verifies email, then
approves the matching code; a remembered browser session usually needs only
approval. The portal also accepts manually entered codes, for another browser
profile or a login session. The browser is assumed to already have the
gateway cookie; no gateway or account token goes in the device URL.

The request lasts ten minutes independently of the initiating connection.
be-host keeps a private secret separately from the displayed code and keeps the
connection that began the request open, with one `finish-browser-login` call
waiting on it until the browser approves or denies the code. Only if that
connection drops does it reconnect, every two seconds, and call again on the
new one. Only that secret can redeem an approved request, once. Approval is
explicit after portal authentication; email verification alone does not
approve a device. A revoked approving browser session cannot issue a be-host
session from an unclaimed approval. The portal authenticates the account;
encryption unlocking and pairing stay in Block.

### Sessions and email

Browser sessions last 30 days and use a host-only HttpOnly SameSite=Lax cookie
scoped to `/auth`; they cannot be used as be-host bearer credentials. be-host's
sessions are listed alongside browser sessions. Revocation and password reset
disconnect active clients and remove their subscriptions. Signing out cannot
remove data already held by a device.

Email delivery uses `mail::Delivery`, currently `LogDelivery`: the server log
contains recipient, subject and the complete message, including verification
and reset links, so the log gives access to the addressed accounts. Tests
inject a capturing delivery sink. Verification links last one hour and password
reset links 30 minutes; tokens are stored hashed and consumed only by POST.
Repeating the requesting form sends another link, subject to issuance limits.
Password reset changes authentication only: encrypted data still needs a
recovery phrase or another unlocked Block device.

### The portal's pages

The page uses server-rendered forms, same-origin POST checks and single-use
form tokens bound to the browser credential. Field validation happens on
submission; browser constraint validation and input length limits are disabled.
Password-manager autocomplete stays enabled, but submitted passwords are not
echoed into HTML after errors. The portal's Referrer-Policy is `strict-origin`:
`no-referrer` makes browser form navigations send a null Origin, which the
same-origin check rejects.

be-server decides what each request shows as a `be_server_portal::Page`
(crates/be-server-portal) and renders it through the `Portal` in
`ServerConfig::portal`. The executable, crates/be-server-bin, passes
crates/be-server-portal-ui: beui styled components rendered with
`render_to_string` on beui's HTML core. Only the executable depends on it, so
the be-server library and its tests build on the native core without beui
(decisions/be_server_portal_ui_only_in_the_executable.md).
`//crates/be-server-bin:bin` builds for `linux_x86_64_html` (`:bin` transitions
there from any configuration, so `:dist`, deploy and be-host's web scripts get
it unchanged). be-server-portal-ui's own tests check what each page renders.

Each page embeds its `Page` as JSON and loads be-server-portal-ui's hydration
bundle, which adds live checks such as mismatched passwords; every form and
link works without it. The executable embeds that bundle (the wasm module, its
JS, the WASI shims and the fonts), so deployment still copies only `be-server`
and `build-info.json`; it serves them under `/auth/assets/HASH/`, HASH from
their contents, with `Cache-Control: public, max-age=31536000, immutable`.
Pages send `Cross-Origin-Opener-Policy: same-origin` and
`Cross-Origin-Embedder-Policy: require-corp`, since the module's memory is
shared, and a Content-Security-Policy that allows only the server's own
scripts, `'wasm-unsafe-eval'` and the import map's hash.

A server with no `Portal`, as every server embedded in a native program is
(be-host's local server, and the servers be-block-frozen's and be-host's tests
and be-host-bench drive the portal of), answers each `/auth` page with its
`Page` as JSON instead, with the same routes, redirects, cookies and form
checks.

To look at the portal locally: `./scripts/buck build //crates/be-server-bin:dist --show-output`,
then run the staged `be-server --allow-login --allow-registration` and open
http://127.0.0.1:9090/auth; verification and reset links are in its log.

### Local profiles

The embedded server alone enables local profiles. Their UI asks only for a
display name, and saved tokens reopen them from the account picker. A saved
token is a local account's only credential, so the picker offers "Remove
account…" where a server account offers "Log out": it asks first, saying the
account's blocks and settings are lost, then logs the session out and forgets
the account as logging out does. Browser-origin requests cannot access
the embedded server's account protocol.

### The default server

`./scripts/deploy` includes `default_server` in every bundle's `build-info.json`:
`name`, `api_url`, `auth_url`, and `gateway_required`. It contains public
configuration only. Ordinary local and CI builds omit it and offer local or
custom servers. Saved accounts keep their own URLs.

## Deploying

`./scripts/deploy` builds be-server, `be` and be-host for every platform in
release in one buck2 command (be-tool-local deploy, in crates/be-tool-local),
checks that its bundles carry the deployed identity and default server, and
then, from that build:

- uploads each `be` bundle (Linux x86_64 and arm64), `be` and its
  `build-info.json`, to the storage zone as `data/BUILD/be-TRIPLE/`
  (`BUNNY_STORAGE_KEY`), reads each back from `be3.pfg.pw` to check the hash,
  and pins both hashes in `scripts/internal/pins`; `./scripts/be` downloads
  both, so its `be` matches the server and knows its default server
  (guides/tracker.md). The pin includes its mirror URL, so the pinned CLI
  keeps its location until a successful deployment writes both the new build
  and `https://be3.pfg.pw/data`;
- uploads be-host's downloads to `data/BUILD/app/` in the same zone, each read
  back the same way: `:native` with its apps for Linux, macOS and Windows on
  x86_64 and arm64 and the web bundle, each zipped on a worker by
  `be-tool-buck archive` into a folder of its own name, and `:android-release`,
  signed locally with the release keystore under the id and label
  `--android-id` and `--android-label` give (`com.be3.block` and `Block`, so
  it installs beside dev's `com.be3.block.dev` and CI's `com.be3.block.ci`), with
  `data/BUILD/app/index.html` linking them. Once the server runs the build (not
  under `--skip-server`), it writes the same page to the unlinked
  `web/downloads.html` with `noindex, nofollow` robots metadata, without reading it back
  through the CDN, and purges it from the CDN. A download connects only to a
  server of its build. `--skip-downloads` leaves the downloads out of the
  build, and `--skip-android` only the APK, so that neither needs the keystore;
- uploads the unpacked web bundle and apps to `data/BUILD/web/` and archives the
  server executable and its build ID to `data/BUILD/server/` in the same zone;
- copies only `be-server` and its `build-info.json` to the VPS over SSH, stops
  be-server, swaps the files and starts it again. Caddy stays running;
- after that succeeds, renders `/index.html` through the shared web-page renderer with a
  `<base href="/data/BUILD/web/">` before its import map and scripts. The browser
  stays at `/` or `/index.html`, keeping query parameters and fragments, so refreshing
  gets the currently deployed build. It uploads the bundle's service worker as
  `/sw.js` beside it, since a service worker only answers for the addresses
  below its own and the page lives at `/`, and purges the web entry point and
  `/sw.js`. `--skip-server` uploads versioned files but leaves both live entry
  points unchanged.

Versioned directories use the Git tree hash that identifies the build. Existing
objects are reused only when their SHA-256 matches; deployment refuses to
overwrite a versioned URL with different bytes. Every versioned artifact is read
back and SHA-256 checked before the live pages are published. Uploading the HTML
entry point last avoids incomplete releases, and long-lived versioned paths keep
modules and apps from different builds from mixing. The web entry point must also return the
isolation headers below; deployment stops before swapping the server if they are
missing. Third-party tool mirrors are on `lfs.pfg.pw`, independent of BE3
deployments.

It refuses a working tree that differs from its commit, so every deployed build
can be built again. Commit the pin it writes. `--dry-run` prints each step and
changes nothing; `./scripts/deploy --help` lists the rest, and the environment
that names the host, the directory there and the storage zone.

### Keys

After the build, Rust loads the storage zone's password (`BUNNY_STORAGE_KEY`,
`be-app-storage-key`) and the account's API key for the purges (`BUNNY_API_KEY`,
`bunny-api-key`), asking at a terminal for missing keys and saving them in
`~/.config/be3`. Deployment checks for the purge key before uploading or
swapping the server and fails if a purge fails; dry runs and `--skip-server`
do not need it. The same local Rust step reads the release keystore
(`android-release-keystore.base64`, or the file `BE3_ANDROID_RELEASE_KEYSTORE`
names), asking at a terminal to generate one or import an existing key when
missing. It adds build metadata and signs the unsigned APK template before
hashing and uploading it; no signing key enters a remote build action. Bash
handles only options that configure the build. Keep a backup of the key:
installed apps accept updates only with the same signing certificate.
`--dry-run` does not read or generate a signing key; it reports APK
finalization and upload without producing them.

### The CDN

These response headers are settings of the Bunny pull zone, not files the
uploader can set in storage:

| Paths | Headers / caching |
| --- | --- |
| `/data/*`, `/`, `/index.html` | `Cross-Origin-Opener-Policy: same-origin`, `Cross-Origin-Embedder-Policy: require-corp` |
| `/data/*` | CDN cache TTL one year; `Cache-Control: public, max-age=31536000, immutable` |
| `/`, `/index.html`, `/sw.js`, `/web/downloads.html` | Override CDN cache TTL to one hour; send `Cache-Control: no-cache` to browsers |

`/data/*` is a prefix rule and the mutable entry points are exact-path rules, so
every later build inherits the cache policy. The mutable pages stay cached at
the edge until deployment purges their exact URLs after uploading replacements;
the CDN TTL is set independently of the browser header, so browsers revalidate
while Bunny serves its cached copy. The zone resolves directory indexes to
`index.html` and preserves uploaded content types, especially `application/wasm`
and JavaScript MIME types.

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

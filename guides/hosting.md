# Hosting be-server

`./scripts/deploy` updates an existing server. The host's systemd service, reverse proxy (Caddy),
data directory and backup schedule are managed on the host, with no templates in this
repository, and deployment leaves them alone.

## Server layout

`BE3_DEPLOY_HOST` (`ubuntu@api.be3.pfg.pw`), `BE3_DEPLOY_DIR` (`be-server`, under the SSH user's
home) and `BE3_DEPLOY_UNITS` (`be-server`, restarted with `sudo systemctl`) name where it goes.
The directory holds only `be-server` and the `build-info.json` it reads beside it.

`--data-dir` holds `metadata.sqlite` and `objects/`. The server adds missing tables, indexes and
defaulted columns on startup and refuses incompatible schemas; the data and its backups are not
frozen (guides/frozen_formats.md), so a change may require deleting them. Don't run `CollectDetached` or `PruneHistory` against
it: be-host never sends them and backups never delete objects.

## Routes, the proxy and the gateway

The server speaks WebSockets at `/api/be` and the same protocol over HTTP posts at
`/api/be/http`, for clients behind proxies that refuse WebSockets. Caddy supplies TLS at
`api.be3.pfg.pw`, routes the API, `/auth` and `/auth/*` to the server and `/build/*` to the build
helper (guides/build_server.md). be-host's web build is served by Bunny at `be3.pfg.pw`.

The gateway accepts `Be3-Gateway-Authorization` from native clients and, from browsers (whose
WebSocket API can't set headers), a host-only cookie on `api.be3.pfg.pw` that visiting
`https://api.be3.pfg.pw/` sets before redirecting to the web build. The public web bundle holds
no gateway key. Because the page is on another origin, the proxy must allow the web build's
exact origin with credentials and let preflights through without the cookie; any HTTP fetch
added for API access must include credentials.

### Proxy account credentials

`Authorization: Bearer <account token>` is reserved for be-server account sessions. A proxy may
inject it into WebSocket upgrades or every HTTP post; the client then calls `authenticate` with
no token, and the server answers with an empty token so the secret stays outside the client.
The Agents dialog's proxy checkbox produces such a bundle (`"authentication": "proxy"`, recovery
phrase kept for decryption).

## Accounts and the portal

Remote accounts are made only through the server's `/auth` portal: password sign-in needs
`--allow-login`, registration also `--allow-registration`. To bootstrap a server, enable
registration, register and verify through the portal, then disable it. A deployed server needs
`--auth-origin https://api.be3.pfg.pw`, the exact origin used for Origin checks, Secure cookies
and email links; request headers can't override it.

Adding a device: be-host starts a ten-minute authorization request and shows `/auth?code=...`;
the user signs in and explicitly approves the code (verifying email alone approves nothing).
be-host holds a private secret apart from the shown code and waits on one
`finish-browser-login` call on the connection that began it, reconnecting only if that drops;
only that secret redeems the approval, once. The portal authenticates the account; unlocking
encryption and pairing stay in be-host.

Browser sessions last 30 days in an HttpOnly cookie scoped to `/auth` and can't be used as
be-host credentials. Email goes through `mail::Delivery`, which is `LogDelivery`: the server log
contains every verification and reset link, so the log gives access to those accounts. A
password reset doesn't unlock encrypted data; that needs the recovery phrase or another unlocked
device.

### The portal's code

The portal is server-rendered forms with same-origin POST checks and single-use form tokens; it
works without scripts. Its Referrer-Policy is `strict-origin` because `no-referrer` makes form
navigations send a null Origin, which the check rejects.

be-server decides each request's `be_server_portal::Page` (crates/be-server-portal) and renders it
through `ServerConfig::portal`. Only the executable (crates/be-server-bin) passes
crates/be-server-portal-ui, beui components rendered to HTML, so the library and its tests build
without beui (decisions/be_server_portal_ui_only_in_the_executable.md); `:bin` transitions to
`linux_x86_64_html` itself. Each page embeds its `Page` as JSON and loads a hydration bundle,
embedded in the executable and served under `/auth/assets/HASH/`; its shared wasm memory is why
pages send COOP/COEP. A server with no `Portal` (every server embedded in a native program or a
test) answers each page with its `Page` as JSON, with the same routes and checks.

To see it locally: `./scripts/buck build //crates/be-server-bin:dist --show-output`, run the
staged `be-server --allow-login --allow-registration` and open http://127.0.0.1:9090/auth;
links are in its log.

### Local profiles and the default server

Only the embedded server offers local profiles, whose saved token is their only credential, so
the account picker offers "Remove account…" instead of "Log out". Browser-origin requests can't
reach the embedded server's account protocol.

`./scripts/deploy` writes `default_server` (`name`, `api_url`, `auth_url`, `gateway_required`)
into every bundle's `build-info.json`; ordinary builds omit it and offer local or custom servers.
A deployed download connects only to a server of its own build.

## Deploying

`./scripts/deploy` (be-tool-local's `deploy`) builds be-server, `be` and be-host for every
platform in release in one buck2 command, then:

1. uploads `be` for Linux x86_64 and arm64 to the Bunny storage zone under `data/BUILD/`, reads
   each back to check its hash, and pins it in `scripts/internal/pins` (guides/tracker.md);
2. uploads be-host's downloads (every desktop platform, the web bundle, and the release-signed
   Android APK, id `com.be3.host`) to `data/BUILD/app/` with an index page, and the unpacked
   web bundle to `data/BUILD/web/`;
3. copies `be-server` and `build-info.json` to the VPS, stops the units, swaps the files and
   starts them;
4. then publishes the mutable entry points: `/index.html` with a `<base href="/data/BUILD/web/">`,
   `/sw.js` beside it (a service worker only answers below its own path), and the unlinked
   `web/downloads.html`, and purges them from the CDN.

`--dry-run` changes nothing; `--skip-server` uploads versioned files only; `--skip-downloads` and
`--skip-android` avoid needing the keystore; `--help` lists the rest. It refuses a dirty working
tree, so every deployment can be rebuilt; commit the pin it writes.

Versioned paths are the build's Git tree hash. Existing objects are reused only when their
SHA-256 matches, and deployment refuses to overwrite a versioned path with different bytes, so
builds never mix. Deployment stops before swapping the server if the web entry point lacks the
isolation headers.

### Keys

After the build, a Rust step loads `BUNNY_STORAGE_KEY`, `BUNNY_API_KEY` (for purges) and the
Android release keystore, asking at a terminal for any that are missing and saving them in
`~/.config/be3`. It signs the APK locally; no key enters a remote build action. Keep a backup of
the keystore: installed apps accept updates only with the same certificate.

### The CDN

These are settings of the Bunny pull zone, not something the uploader can set:

| Paths | Headers and caching |
| --- | --- |
| `/data/*`, `/`, `/index.html` | `Cross-Origin-Opener-Policy: same-origin`, `Cross-Origin-Embedder-Policy: require-corp` |
| `/data/*` | CDN TTL one year; `Cache-Control: public, max-age=31536000, immutable` |
| `/`, `/index.html`, `/sw.js`, `/web/downloads.html` | CDN TTL one hour; `Cache-Control: no-cache` to browsers |

The zone resolves directory indexes and keeps uploaded content types (`application/wasm`
especially).

## Backups and restoring

Backups are scheduled on the host; `be-server --help` describes the commands.

- `be-server backup-key > backup.key` makes the key. Keep a copy off the VPS: without it nothing
  can be restored.
- `be-server backup --data-dir DATA --to bunny:ZONE --key-file backup.key`, with
  `BUNNY_STORAGE_KEY` and `BUNNY_STORAGE_ENDPOINT` (such as `https://ny.storage.bunnycdn.com`).
  That password can delete what it wrote, so replicate the zone and copy backups somewhere the
  VPS can't reach.
- To restore: stop be-server, run
  `be-server restore --from bunny:ZONE --key-file backup.key --data-dir NEW-DIR` (`--snapshot
  NAME` for an older one), then `be-server verify --data-dir NEW-DIR`, point the service at it
  and start it. Devices reconnect with the tokens they had; notes still need a recovery phrase
  or an unlocked device to read.

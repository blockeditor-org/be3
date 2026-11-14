# Hosting be-server

`./scripts/deploy` updates the be-server binary on the server. Everything else on it (the
systemd unit, Caddy and its gateway, the data directory on the volume) is its NixOS
configuration, `servers/api` (guides/servers.md).

## Server layout

`BE3_DEPLOY_HOST` (`root@api.be3.pfg.pw`), `BE3_DEPLOY_DIR` (`/srv/be-server`) and
`BE3_DEPLOY_UNITS` (`be-server`, restarted with `sudo systemctl`) name where it goes. The
directory holds only `be-server` and the `build-info.json` it reads beside it. The unit runs it
as the user `be-server` with `--data-dir /mnt/data/be-server`, on the volume, listening on
127.0.0.1:9090 only.

`--data-dir` holds `metadata.sqlite` and `objects/`, or `object-cache/` when objects live in a
bucket (below). The server adds missing tables, indexes and
defaulted columns on startup and refuses incompatible schemas; the data and its backups are not
frozen (guides/frozen_formats.md), so a change may require deleting them. Don't run `CollectDetached` against
it: be-host never sends it. Publishing prunes history as commits arrive (guides/blocks/server.md);
backups keep every object they copied.

## Objects in a bucket

Objects can live in an S3-compatible bucket instead of `objects/`: set all of `BE_S3_ENDPOINT`,
`BE_S3_REGION`, `BE_S3_BUCKET`, `BE_S3_ACCESS_KEY` and `BE_S3_SECRET_KEY` in the service's
environment (some but not all is an error). The server keeps the objects it used most recently
in `object-cache/`, `--object-cache BYTES` of them (2 GiB by default); deleting that directory
loses nothing. `backup`, `restore` and `verify` read the same variables, so run them with the
service's environment.

For a Bunny storage zone, through its S3 API: the endpoint is the zone's region's S3 host (shown
with the zone's S3 credentials in the dashboard), the bucket is the zone's name, the access key
the zone's name too and the secret key its password; the region is whatever the dashboard
names. Requests are path-style, so no per-zone hostname is needed. Use a zone of its own, not
the one deployments upload to: the server lists and deletes keys in it. Moving an existing
server's objects there is a backup and a restore into a fresh data directory with the variables
set.

With a bucket, clients upload and download objects with URLs the server presigns against the
bucket, so the bucket must answer browsers too: give the zone a CORS rule allowing the web
build's origin (`https://be3.pfg.pw`) to `GET`, `HEAD` and `PUT`, with the request headers
`content-type`, `range` and `x-amz-checksum-sha256` (a PUT that sends the checksum is
preflighted) and exposing `Content-Length`, `Content-Range` and `ETag`. Requests carry no
credentials; the signature is the authority. Without a bucket, the server's own S3 endpoint
answers these itself.

## Listening

Nothing in the repository listens on every interface unless whoever runs it asks. Every
server, script and harness listens on loopback by default: be-server's `--addr` is
`127.0.0.1:9090` and the build helper's `127.0.0.1:9091`, the scripts that serve web bundles
take `--addr` with a `127.0.0.1` default, and tests and `:dev` targets bind `127.0.0.1` (port
0 where they can). The one way to listen elsewhere is to give that address to `--addr`, the
same flag on each of them: `--addr 0.0.0.0:PORT` (or `[::]:PORT`) to reach a server or a web
build from another machine. `//crates/be-host:web`'s `--domain` serves a public deployment on
ports 80 and 443 of every interface. No test, `:dev` target, CI job or verify step passes either.

Caddy listens on every interface whatever host a site address names; only `default_bind` (or
`bind`) narrows it. So each Caddyfile run on this machine or in a test sets
`default_bind {$NAME_BIND:127.0.0.1}` in its global options, which falls back to loopback, and
the script that runs it sets `NAME_BIND` to the host of its `--addr`. The site matches the Host
header too, so a page on another origin that rebinds its name to 127.0.0.1 is not answered:

- on loopback, `http://127.0.0.1:PORT, http://localhost:PORT`;
- on any other single address, such as a LAN IP, only `http://HOST:PORT`, so clients must use
  that address, not a hostname that resolves to it;
- on a wildcard `--addr`, `http://:PORT`, any Host.

The build server's Caddyfile (scripts/internal/nativelink) is its front door on the internet and
binds every interface on purpose.

`./scripts/verify` enforces this (crates/be-tool-verify, `binds.rs`). It fails on a Caddyfile
outside scripts/internal without a loopback `default_bind`; on `0.0.0.0`, `Addr::UNSPECIFIED`,
or `[::]` written as an address (`[::]:PORT`, quoted, or after `=`), outside a comment, a guide,
scripts/internal or the lines `ALLOWED_LINES` names (`--domain`'s bind, and the scripts' check
for a wildcard `--addr`); and on a Node `.listen(` line that names no loopback host, which
listens on every interface.

## Routes, the proxy and the gateway

The server speaks WebSockets at `/api/be` and the same protocol over HTTP posts at
`/api/be/http`, for clients behind proxies that refuse WebSockets; signing in through the
browser is one-shot posts to `/api/be/call`. Caddy (`servers/api/Caddyfile`) supplies TLS at
`api.be3.pfg.pw` and passes everything with the gateway key to the server; nothing serves
`/build/*` (the build helper, guides/build_server.md) there now. be-host's web build is served
by Bunny at `be3.pfg.pw`.

be-server takes a client's address from `X-Forwarded-For` only when the connection comes from a
trusted proxy: loopback, where Caddy runs, or an address named with `--trusted-proxy`. Anyone
else's header is ignored, so the server can also face the internet directly. Limits per address
(connections, HTTP sessions, one-shot calls, pending sign-ins; guides/blocks/server.md,
"Limits") count by that
address, so a proxy elsewhere must be named or every client counts as the proxy.

be-server is not yet trusted to face the internet, so the gateway passes a request on only
with the key, `API_GATEWAY_KEY` in `/mnt/data/secrets/gateway.env` on the server (made once, on
the volume). It accepts `Be3-Gateway-Authorization` from native clients and, from browsers
(whose WebSocket API can't set headers), the host-only cookie `be3_gateway` on `api.be3.pfg.pw`:
the page at `https://api.be3.pfg.pw/` asks for the key, sets the cookie and goes on to the web
build, and a browser sent anywhere else without it is sent to that page and back. Without the
key everything else is 401. The public web bundle holds no gateway key; the tracker's CI has it
as `BE3_TRACKER_AUTHORIZATION`, and `./scripts/be` as `BE3_AUTHORIZATION`. Because the page is on another origin, the proxy must allow the web build's
exact origin with credentials and let preflights through without the cookie; any HTTP fetch
added for API access must include credentials. The S3 endpoint, `/api/be/storage/*`
(guides/blocks/server.md, "Objects over S3"), is the exception: its presigned URLs are fetched
without credentials, so the proxy must pass it through without the gateway check, keeping its
path, `Host` and query (the signature covers them) and adding no CORS headers of its own. The
server signs those URLs for the URL clients reach it at, which it can't see behind Caddy: run
it with `--public-url https://api.be3.pfg.pw/api/be`, or every presigned URL names its
loopback address.

### Proxy account credentials

`Authorization: Bearer <account token>` is reserved for be-server account sessions. A proxy may
inject it into WebSocket upgrades or every HTTP post; the client then calls `authenticate` with
no token, and the server answers with an empty token so the secret stays outside the client.
The Agents dialog's proxy checkbox produces such a bundle (`"authentication": "proxy"`, recovery
phrase kept for decryption).

## Accounts and the portal

Remote accounts are made only through the server's `/auth` portal: password sign-in needs
`--allow-login`, registration also `--allow-registration` and an invite code. Codes are made on
the server against the data directory (`sudo -u be-server /srv/be-server/be-server invite-code
... --data-dir /mnt/data/be-server`), which works while the server runs:

```
be-server invite-code create --data-dir DATA [--uses N] [--expires DAYS]   # prints the code once
be-server invite-code list --data-dir DATA
be-server invite-code revoke --data-dir DATA ID
```

The server stores only a code's hash; a registration spends one use when its email is verified,
and revoking a code also stops registrations still waiting on their email. To bootstrap a
server, make a code with `--uses 1`, start it with `--allow-registration`, register and verify
through the portal with the code, then restart it without `--allow-registration`. A deployed server needs
`--auth-origin https://api.be3.pfg.pw`, the exact origin used for Origin checks, Secure cookies
and email links; request headers can't override it.

Adding a device: be-host begins a ten-minute authorization request with a one-shot post to
`/api/be/call` and shows `/auth?code=...`; the user signs in and explicitly approves the code
(verifying email alone approves nothing). be-host holds a private secret apart from the shown
code and polls with it, one short post at a time (every ten seconds, or as soon as the server
allows when the user presses Check now), holding no connection meanwhile; only that secret
redeems the approval, once, and be-host then connects its websocket with the token. The web
build's posts are `fetch`es with credentials and no content type, so they carry the gateway
cookie and need no preflight. The portal authenticates the account; unlocking encryption and
pairing stay in be-host.

Each account may store 10 GiB by default, counting every version its blocks keep and uploads
no block has taken yet; `--account-quota BYTES` changes it and `--account-quota none` lifts it.
Uploads no block takes are deleted after a day.

Push notifications (guides/blocks/server.md, "Notifications") go out through Web Push with a
VAPID key the server makes in its database, to the known push services only (FCM, Mozilla,
Apple, Windows). The key is plaintext inside the database, so inside its backups, which are
sealed with the backup key, and has no rotation path. A self-hosted UnifiedPush server such as
ntfy needs `--push-host HOST`, once per host. The server must reach them over IPv4 or native IPv6: behind DNS64, an IPv4-only push
service resolves into `64:ff9b::/96`, which the server refuses as not public. FCM, for Android devices
without a UnifiedPush distributor, needs a Firebase project: its service account's JSON key on
the host, passed as `--fcm-credentials FILE`. The deployment has none yet, so the server offers
only Web Push.

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
staged `be-server --allow-login --allow-registration` and open http://127.0.0.1:9090/auth,
registering with a code from `be-server invite-code create`; links are in its log.

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
   every download's `build-info.json` (the APK's too) names each `be`'s URL and SHA-256
   (`terminal_hosts`), so the host installs a terminal host of its own build
   (guides/terminal.md, A terminal host in a boat.dev sandbox);
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

Nothing schedules backups yet: the volume is the only copy. `be-server --help` describes the
commands; on the server, run them as `be-server` with the binary in `/srv/be-server`.

- `be-server backup-key > backup.key` makes the key. Keep a copy off the VPS: without it nothing
  can be restored.
- `be-server backup --data-dir DATA --to bunny:ZONE --key-file backup.key`, with
  `BUNNY_STORAGE_KEY` and `BUNNY_STORAGE_ENDPOINT` (such as `https://ny.storage.bunnycdn.com`).
  That password can delete what it wrote, so replicate the zone and copy backups somewhere the
  VPS can't reach.
- With objects in a bucket, a backup reads them from the bucket (through the cache) and a
  restore writes them to the bucket the variables name.
- To restore: stop be-server, run
  `be-server restore --from bunny:ZONE --key-file backup.key --data-dir NEW-DIR` (`--snapshot
  NAME` for an older one), then `be-server verify --data-dir NEW-DIR`, point the service at it
  and start it. Devices reconnect with the tokens they had; notes still need a recovery phrase
  or an unlocked device to read.

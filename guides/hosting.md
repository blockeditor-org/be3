# Hosting be-server

How the server runs on a VPS for real notes. The files referred to are in
`crates/be-server/deploy`.

## Layout

- be-server listens on `127.0.0.1:9090` and is never exposed directly: it
  speaks plain websockets with no TLS, and the same protocol over HTTP posts
  at `/api/be/http` for clients behind a proxy that refuses websockets (`be`).
- Caddy can gate the site on `Be3-Gateway-Authorization`;
  the app (Gateway authorization header when adding an account on a remote server) and
  `be` (`BE3_AUTHORIZATION`) send one.  `Caddyfile.gated`
  is the site behind such a key: a browser cannot put a header on a websocket,
  so anyone else gets a page that asks for the key and keeps it in a cookie,
  which Caddy accepts as well, and the key never goes in a URL.
- Caddy terminates TLS at `api.be3.pfg.pw` and passes `/api*` to be-server.
  It allows browser requests from `https://be3.pfg.pw`; native clients send no
  Origin header. Its root redirects to `https://be3.pfg.pw/web` after the gateway
  cookie is established. The templates are `Caddyfile` and `Caddyfile.gated`.
- Bunny's `be-app` storage zone (Los Angeles primary, New York replica) and
  `be-app` pull zone serve deployment artifacts at `https://be3.pfg.pw`.
  The browser connects directly to `wss://api.be3.pfg.pw/api/be`; the gateway
  cookie belongs only to that API host, not to the public static files.
- Data lives in `/var/lib/be-server`: `metadata.sqlite` and `objects/`. On
  start the server checks `metadata.sqlite` against the schema it declares,
  adding missing tables, indexes and defaulted columns, and refuses to start on
  anything else. The directory and its backups are frozen formats
  (guides/frozen_formats.md): every later server reads them.
- Sign-ins with a password are off, so only devices that are already signed
  in connect, with the session tokens they hold. To sign in a new device,
  put `BE_SERVER_FLAGS=--allow-login` in `/etc/be-server/server.env`, restart
  be-server, sign in, then empty that line and restart again. Restarting drops
  every connection for a moment; devices reconnect on their own.
- Registration is closed. Accounts are made on the server:
  `sudo -u be-server be-server --data-dir /var/lib/be-server --add-account EMAIL NAME ORGANIZATION`,
  typing the password on standard input. It writes the database directly, so
  stop the service while it runs.

## Install

1. Build `//crates/be-server:dist` for the VPS
   (`./scripts/buck build //crates/be-server:dist --out DIR`), copy `DIR/be-server`
   to `/usr/local/bin/be-server` and `DIR/build-id` to `/etc/be-server/build-id`.
   The server refuses clients of any other build, so upgrade it together with
   the web bundle and every device, all from one commit.
2. `useradd --system --home /var/lib/be-server be-server`.
3. Copy `be-server.service`, `be-backup.service` and `be-backup.timer` to
   `/etc/systemd/system/`, then `systemctl enable --now be-server be-backup.timer`.
4. Put `Caddyfile` (with the real domain) in Caddy's config and reload it.
5. `journalctl -u be-server` shows its log; `RUST_LOG=debug` in the unit shows
   each connection.

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
  it installs beside CI's `com.be3.block.ci`), with
  `data/BUILD/app/index.html` linking them. Once the server runs the build (not
  under `--skip-server`), it writes the same page to `index.html`, the one
  people are sent to (`be3.pfg.pw/`), without reading it back
  through the CDN, and purges it from the CDN using the required `BUNNY_API_KEY`. A
  download connects only to a server of its build. `--skip-downloads` leaves
  the downloads out of the build, and `--skip-android` only the APK, so that
  neither needs the keystore;
- uploads the unpacked web bundle and plugins to `data/BUILD/web/` and archives the
  server executable and its build ID to `data/BUILD/server/` in the same zone;
- copies only `be-server` and its `build-id` to the VPS over SSH, stops
  be-server, swaps the files and starts it again. Caddy stays running;
- after that succeeds, publishes `/web`, the matching app HTML with a
  `<base href="/data/BUILD/web/">` before its import map and scripts. The browser
  stays at `/web`, keeping query parameters and fragments, so refreshing
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
| `/data/*`, `/web` | `Cross-Origin-Opener-Policy: same-origin`, `Cross-Origin-Embedder-Policy: require-corp` |
| `/data/*` | CDN cache TTL one year; `Cache-Control: public, max-age=31536000, immutable` |
| `/`, `/index.html`, `/web` | Override CDN cache TTL to one hour; send `Cache-Control: no-cache` to browsers |

The mutable pages stay cached at the edge until deployment purges their exact URLs after uploading replacements. Configure the CDN TTL independently of the browser header, so browsers revalidate while Bunny can serve its cached copy. Deployment checks for the purge key before uploading or swapping the server and fails if a purge fails. Dry runs and `--skip-server` do not require the purge key.

Use a prefix rule for `/data/*` and exact-path rules for the three mutable entry points, so every later build inherits the cache policy. Enable directory index resolution to `index.html`. Preserve uploaded content
types, especially `application/wasm` and JavaScript MIME types. Uploading an
HTML entry point last avoids incomplete releases; long-lived versioned paths
avoid mixing modules and plugins from different builds. The public web bundle
contains no gateway key. Visit `https://api.be3.pfg.pw/` to unlock the gateway;
after setting its host-only cookie it redirects to the web app. Keep the same
gateway secret when adding the new API hostname, and establish a new cookie
there; the old host's cookie does not move between hosts.

Before building it finds the keys it needs besides the build server's,
asking at a terminal for any it lacks and saving them in `~/.config/be3`:
the storage zone's password (`BUNNY_STORAGE_KEY`, `be-app-storage-key`), the
account's API key for the purge (`BUNNY_API_KEY`, `bunny-api-key`, required when publishing live pages), and the release keystore (`android-release-keystore.base64`),
which it can make anew and copies to `buck/android/release-keystore.base64`
for the signing action. Android installs an update only when it is signed
with the same key, so keep a copy of the keystore off the machine: a new one
means every device uninstalls the app once. Like CI's, it is an input of the
signing action, so it is in the build server's CAS.

It refuses a working tree that differs from its commit, so every deployed build
can be built again. Commit the pin it writes. `--dry-run` prints each step and
changes nothing; `./scripts/deploy --help` lists the rest, and the environment
that names the host, the directory there and the storage zone.

## Backups

`be-backup.timer` runs `be-server backup` hourly and `be-server verify` after
it. `be-server --help` describes what a backup holds and keeps.

- `be-server backup-key > /etc/be-server/backup.key`, readable only by
  `be-server`. Keep a copy off the VPS: a backup cannot be restored without it.
- `/etc/be-server/backup.env` holds `BUNNY_STORAGE_ZONE`, `BUNNY_STORAGE_KEY`
  (the zone's password) and `BUNNY_STORAGE_ENDPOINT` (the zone's region, such
  as `https://ny.storage.bunnycdn.com`).
- The zone's password can delete what it wrote, so a compromised VPS can delete
  its backups. Turn on the zone's replication to a second region, and now and
  then copy the zone to a machine the VPS cannot reach.
- Notes are end-to-end encrypted. A restored server serves them, but reading
  them still needs the account's recovery phrase or a device the account has
  unlocked.

## Restoring

1. Stop be-server.
2. `be-server restore --from bunny:ZONE --key-file backup.key --data-dir NEW-DIR`
   restores the newest database and every object it references into an empty
   directory. `--snapshot NAME` picks an older one from `databases/`.
3. `be-server verify --data-dir NEW-DIR`, then point the service at it (or move
   it to `/var/lib/be-server`) and start it. Devices reconnect with the tokens
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
`Be3-Gateway-Authorization`; existing Caddy configuration and proxy rules must
switch together. Browsers use the gateway cookie because their WebSocket API
cannot set request headers.

The app embeds `crates/block-app/src/app-config.json` in both native and web builds. Its `account_server` prefills the server address when adding an account; self-hosted builds should set it to their API URL. Saved accounts keep their own server address. This configuration is independent of the `build-id` compatibility stamp.

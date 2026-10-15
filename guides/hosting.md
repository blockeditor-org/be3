# Hosting be-server

How the server runs on a VPS for real notes. The files referred to are in
`crates/be-server/deploy`.

## Layout

- be-server listens on `127.0.0.1:9090` and is never exposed directly: it
  speaks plain websockets with no TLS, and the same protocol over HTTP posts
  at `/api/be/http` for clients behind a proxy that refuses websockets (`be`).
- It never reads `Authorization`, so Caddy can gate the site on that header;
  the app (Authorization header when adding an account on a remote server) and
  `be` (`authorization=` in `external/.tracker-credentials`) send one.  `Caddyfile.gated`
  is the site behind such a key: a browser cannot put a header on a websocket,
  so anyone else gets a page that asks for the key and keeps it in a cookie,
  which Caddy accepts as well, and the key never goes in a URL.
- Caddy terminates TLS for the domain, passes `/api*` to be-server, and serves
  the web bundle from `/srv/be3-web` if it is wanted (`Caddyfile`; build it
  with `./scripts/buck build //crates/block-app:web --out DIR`).
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
  `sudo -u be-server be-server --data-dir /var/lib/be-server --add-account EMAIL NAME WORKSPACE`,
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
  `be/BUILD/be-TRIPLE/` (`BUNNY_STORAGE_KEY`), reads it back from
  `lfs.pfg.pw` to check the hash, and pins it in `scripts/internal/common.sh`,
  which `./scripts/be` must match the server by (guides/tracker.md);
- uploads the app's downloads to `app/BUILD/` in the same zone, each read
  back the same way: `:native` with its plugins for Linux, macOS and Windows on
  x86_64 and arm64 and the web bundle, each zipped on a worker by
  `buck-tools archive` into a folder of its own name, and `:android-release`,
  signed on a worker with the release keystore under the id and label
  `--android-id` and `--android-label` give (`com.be3.block` and `Block`, so
  it installs beside CI's `com.be3.block.ci`), with
  `app/BUILD/index.html` linking them. Once the server runs the build, it
  writes the same page to `app/index.html`, the one people are sent to
  (`lfs.pfg.pw/by-name/be3/app/`), and purges it from the CDN when
  `BUNNY_API_KEY` is set. A download connects only to a server of its build;
- copies `be-server`, its `build-id` and the web bundle (`//crates/block-app:web`,
  with every plugin) to the VPS beside the running ones, stops Caddy and
  be-server, moves them into place (the bundle to `/srv/be3-web`, keeping the
  last one as `/srv/be3-web.old`) and starts both again. The app, the web app
  and `be` are then all one build, which the server requires.

Before building it finds the keys it needs besides the build server's,
asking at a terminal for any it lacks and saving them in `~/.config/be3`:
the storage zone's password (`BUNNY_STORAGE_KEY`, `bunny-storage-key`), the
account's API key for the purge (`BUNNY_API_KEY`, `bunny-api-key`, which may
be left empty), and the release keystore (`android-release-keystore.base64`),
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
  them still needs the account's recovery phrase or a device that holds the
  workspace key.

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

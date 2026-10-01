# Hosting be-server

How the server runs on a VPS for real notes. The files referred to are in
`crates/be-server/deploy`.

## Layout

- be-server listens on `127.0.0.1:9090` and is never exposed directly: it
  speaks plain websockets with no TLS.
- Caddy terminates TLS for the domain, passes `/api*` to be-server, and serves
  the web bundle from `/srv/be3-web` if it is wanted (`Caddyfile`; build it
  with `./scripts/buck build //crates/block-app:web --out DIR`).
- Data lives in `/var/lib/be-server`: `metadata.sqlite` and `objects/`.
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

1. Build `//crates/be-server:be-server-bin` for the VPS
   (`./scripts/buck build //crates/be-server:be-server-bin --out be-server`) and
   copy it to `/usr/local/bin/be-server`.
2. `useradd --system --home /var/lib/be-server be-server`.
3. Copy `be-server.service`, `be-backup.service` and `be-backup.timer` to
   `/etc/systemd/system/`, then `systemctl enable --now be-server be-backup.timer`.
4. Put `Caddyfile` (with the real domain) in Caddy's config and reload it.
5. `journalctl -u be-server` shows its log; `RUST_LOG=debug` in the unit shows
   each connection.

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

# The build server

Every buck2 action runs on our own remote execution server at
https://blocks.pfg.pw, which `.buckconfig` names. Running `./scripts/build-server
install` on a machine sets it up there. A key keeps everyone else out.

## Building on it

`./scripts/buck` needs the key. It looks for it in this order:

1. `BE3_BUILD_SERVER_KEY` in the environment
2. `.build-server-key` at the root of the checkout
3. `~/.config/be3/build-server-key`

With none of them, it asks for the key at a terminal and saves it to
`~/.config/be3/build-server-key`.

- **The machine running the server:** `install` writes the key there for you.
- **CI:** it reads the key from the repository secret `BE3_BUILD_SERVER_KEY`.
  Set it with `gh secret set BE3_BUILD_SERVER_KEY`.
- **A call without the key:** Caddy refuses it with gRPC's `UNAUTHENTICATED`.

When `HTTPS_PROXY` is set, buck2 reaches the server through
`scripts/internal/re-relay` (guides/buck2.md).

## Setting up a new server

You need:

- an Ubuntu machine (24.04 or newer) with sudo
- a domain whose A record points at the machine
- ports 80 and 443 open, so Caddy can get a certificate and serve
- free disk: about 1 GB for the image, up to 35 GB for the cache, and room
  for actions' work directories

Then:

1. Clone the repository on the machine.
2. Run `./scripts/build-server install DOMAIN`. Running it again is safe: it
   changes only what differs, and restarts only what it changed. It:
   - installs `busybox-static`, `uidmap` and `caddy` if they are missing
   - adds the system user `be3-build` with subordinate group IDs
   - downloads NativeLink, pinned by hash, into `/var/lib/be3-build-server`
   - unpacks the workers' container image there too
   - writes the systemd service `be3-build-server`
   - generates a key into `/etc/be3-build-server/key`, and copies it to the
     key file for the user who ran `install`
   - replaces `/etc/caddy/Caddyfile` with the server's site. A Caddyfile it
     did not write is first kept as `Caddyfile.before-build-server`.
3. If the domain is new, change it in three places:
   - the `[buck2_re_client]` addresses in `.buckconfig`
   - the example host in `scripts/internal/re-relay/main.go`
   - this guide
4. Give the key to every machine that builds, and to CI as the secret
   `BE3_BUILD_SERVER_KEY`.

The cache starts empty, so the first build of everything takes a while.
`//:check` took about 16 minutes on six cores.

## How it works

- **NativeLink** provides the CAS, the action cache, the scheduler and one
  worker in a single process. It listens on `127.0.0.1:50052`, and its worker
  API on `127.0.0.1:50062`.
- **Concurrency.** The worker runs as many actions at once as the machine has
  cores.
- **Cache size.** The CAS keeps 35 GB.
- **Evicted outputs.** The action cache reports a result as a miss once the
  CAS has evicted that result's outputs.
- **Actions run as root in the workers' image.** That image is
  `worker_properties` in `buck/tools/defs.bzl`.
  `scripts/internal/build-server-action.sh` enters it with a user namespace and
  a chroot, not Docker.
- **The user namespace.** Root in the namespace is `be3-build` outside it.
  Groups map to `be3-build`'s subordinate groups, so `dpkg-deb` can give files
  any group.
- **AppArmor.** Ubuntu lets only programs that AppArmor names create user
  namespaces. `install` adds a root-owned copy of `unshare` at
  `/usr/local/libexec/be3-build-server/unshare`, with a profile that allows
  only that copy.
- **The entrypoint.** NativeLink starts each action through `run-action`, a
  static busybox script. A test's `LD_LIBRARY_PATH` names the sysroot's glibc,
  so `run-action` holds `LD_*` back until the action itself starts.

## Looking after it

- **Logs:** `journalctl -u be3-build-server` for the server,
  `journalctl -u caddy` for Caddy.
- **When `worker_properties` changes the image:** run `install` again. It
  unpacks the new image.
- **New key:** delete `/etc/be3-build-server/key`, run `install` again, then
  hand the new key out.
- **Empty the cache:** stop the service, delete `cas`, `ac` and `work` in
  `/var/lib/be3-build-server`, and start the service again. Every client must
  also run `./scripts/buck kill`, because buck2 remembers which blobs it
  uploaded.
- **What the key gives away.** Anyone with the key can run any command on the
  machine as `be3-build`.

The server holds no secrets for actions, so CI signs its APKs on the runner
(guides/buck2.md).

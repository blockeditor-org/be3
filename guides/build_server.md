# The build server

`./scripts/build-server` runs our own remote execution server in BuildBuddy's
place. It is at https://blocks.pfg.pw. buck2 sends it every action it would
send BuildBuddy, and a key keeps everyone else out.

## Building on it

- `./scripts/build-server use blocks.pfg.pw` asks for the key, then points
  every checkout on the machine at the server. It keeps the domain in
  `~/.config/be3/build-server` and the key in `~/.config/be3/build-server-key`.
- `./scripts/build-server off` points them back at BuildBuddy.
- `./scripts/build-server status` says which one this machine uses.
- `BE3_BUILD_SERVER=DOMAIN` and `BE3_BUILD_SERVER_KEY=KEY` in the environment
  do the same for one command.
- CI builds on the server while the repository variable `BE3_BUILD_SERVER`
  names one. It reads the key from the secret `BE3_BUILD_SERVER_KEY`:
  `gh variable set BE3_BUILD_SERVER --body blocks.pfg.pw` and
  `gh secret set BE3_BUILD_SERVER_KEY`. To return CI to BuildBuddy, run
  `gh variable delete BE3_BUILD_SERVER`.

`./scripts/buck` then writes a `.buckconfig.local`. It holds the server's
address and an `authorization: Bearer` header, so `.buckconfig` no longer
reaches the daemon's BuildBuddy settings. When `HTTPS_PROXY` is set, the
address goes through `scripts/internal/re-relay`, just as BuildBuddy's would.

## Setting up a new server

You need:

- an Ubuntu machine (24.04 or newer) with sudo
- a domain whose A record points at the machine
- ports 80 and 443 open, so Caddy can get a certificate and serve
- free disk: about 1 GB for the image, up to 20 GB for the cache, and room
  for actions' work directories

Then:

1. Clone the repository on the machine.
2. Run `./scripts/build-server install DOMAIN`. Running it again is safe: it
   changes only what differs, and restarts only what it changed. It:
   - installs `busybox-static`, `uidmap` and `caddy` if they are missing
   - adds the system user `be3-build` with subordinate group IDs
   - downloads NativeLink, pinned by hash, into `/var/lib/be3-build-server`
   - unpacks the workers' container image next to it
   - writes the systemd service `be3-build-server`
   - generates a key into `/etc/be3-build-server/key`
   - replaces `/etc/caddy/Caddyfile` with the server's site. Anything that file
     held before is kept as `Caddyfile.before-build-server`.
   - points the machine's own checkouts at the server
3. Give the key to the machines and to CI as described above. Point
   `BE3_BUILD_SERVER` at the new domain, and change the domain in this guide.

The cache starts empty, so the first build of everything takes a while.
`//:check` took about six minutes on six cores.

## How it works

- **NativeLink** provides the CAS, the action cache, the scheduler and one
  worker in a single process. It listens on `127.0.0.1:50052`, and its worker
  API on `127.0.0.1:50062`.
- **Concurrency.** The worker runs as many actions at once as the machine has
  cores.
- **Cache size.** The CAS keeps 20 GB.
- **Evicted outputs.** The action cache reports a result as a miss once the
  CAS has evicted that result's outputs.
- **Actions run as root in the workers' image.** That image is
  `worker_properties` in `buck/tools/defs.bzl`; an action sees the same
  system it would see on BuildBuddy, and runs as root as it would there.
  `scripts/internal/build-server-action.sh` enters the image with a user
  namespace and a chroot, not Docker.
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
- **Caddy** turns away any call without the key. It answers with gRPC's
  `UNAUTHENTICATED`, so buck2 says what is wrong.

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
- **The Android signing secret** (`env-secrets`) exists only on BuildBuddy.
  Here `:android-dist` signs without it.
- **What the key gives away.** Anyone with the key can run any command on the
  machine as `be3-build`.

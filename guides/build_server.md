# The build server

Every buck2 action runs on our own remote execution server at
https://blocks.pfg.pw, which `.buckconfig` names. Only callers with the key
can use it.

## Building on it

`./scripts/buck` needs the key. It looks for it in this order:

1. `BE3_BUILD_SERVER_KEY` in the environment
2. `.build-server-key` at the root of the checkout
3. `~/.config/be3/build-server-key`

With none of them, it asks for the key at a terminal and saves it to
`~/.config/be3/build-server-key`.

- **CI:** it reads the key from the repository secret `BE3_BUILD_SERVER_KEY`.
  Set it with `gh secret set BE3_BUILD_SERVER_KEY`.
- **A call without the key:** Caddy refuses it with gRPC's `UNAUTHENTICATED`.

When `HTTPS_PROXY` is set, buck2 reaches the server through
`scripts/internal/re-relay` (guides/buck2.md).

A proxy can supply the key itself. In that case it adds
`Authorization: Bearer KEY` to requests for the server's host, and
`BE3_BUILD_SERVER_KEY` can be any placeholder, such as `proxy-injected`. The
server accepts a call if any of its `Authorization` headers carries the key,
so it doesn't matter whether the proxy replaces buck2's placeholder header or
adds its own beside it.

## How it works

- **NativeLink** provides the CAS, the action cache, the scheduler and one
  worker in a single process. It runs as the systemd service
  `be3-build-server`, as the unprivileged user `be3-build`, and keeps
  everything in `/var/lib/be3-build-server`. It listens on `127.0.0.1:50052`,
  and its worker API on `127.0.0.1:50062`.
- **Caddy** terminates TLS for the domain and forwards a call only when it
  carries `Authorization: Bearer KEY`.
- **Actions run as root in the workers' image.** That image is
  `worker_properties` in `buck/tools/defs.bzl`; an action sees the same
  system wherever it runs, and some actions need root. `action.sh` enters the
  image with a user namespace and a chroot, not Docker. Each action gets its
  own `/tmp` and `/root`.
- **The user namespace.** Root in the namespace is `be3-build` outside it.
  Groups map to `be3-build`'s subordinate groups, so `dpkg-deb` can give files
  any group.
- **AppArmor.** Ubuntu lets only programs that AppArmor names create user
  namespaces. A root-owned copy of `unshare` at
  `/usr/local/libexec/be3-build-server/unshare` has a profile that allows
  only that copy.
- **The entrypoint.** NativeLink starts each action through `run-action`, a
  static busybox script. A test's `LD_LIBRARY_PATH` names the sysroot's glibc,
  which none of the programs on the way into the image can load, so
  `run-action` holds `LD_*` back until the action itself starts.
- **Evicted outputs.** The action cache reports a result as a miss once the
  CAS has evicted that result's outputs.
- **Cache size.** One of CI's builds needs more than 35 GB of cache at once. A
  blob evicted partway through a build is an input buck2 thinks it already
  uploaded, and the actions that need it fail with
  `not found in either fast or slow store`.

## Secrets

`/etc/be3-build-server/secrets/NAME` holds a value an action can ask for. Only
`be3-build` can read it.

How an action receives a secret:

1. The action names the secrets it wants in its `BE3_BUILD_SERVER_SECRETS`
   environment variable, separated by commas.
2. `run-action` exports each one as a variable of that name.

NativeLink's worker never sees buck2's platform properties, because buck2 puts
them in the Command and not the Action. So a platform property can't carry
this request; it goes in the environment.

The only secret today is CI's Android keystore, `ANDROID_DEBUG_KEYSTORE_BASE64`.
`:android-dist` (`signed_apk` in `buck/android/defs.bzl`) signs CI's APKs with
it, and every build is signed with the same keystore so that each one installs
as an update over the last.

- Moving to a new server means copying the file from the old one. A new
  keystore means every device must uninstall the CI app and launcher once.
- The secret is not part of the action's key. After it changes, bump
  `key_version` in `crates/block-app/BUCK` and `crates/be-launcher/BUCK`.

## Setting up a new server

You need:

- an Ubuntu machine (24.04 or newer) with sudo
- a domain whose A record points at the machine
- ports 80 and 443 open, so Caddy can get a certificate and serve
- 100 GB or more of disk for `/var/lib/be3-build-server`

The steps below use these shell variables; set them first:

```
server=/var/lib/be3-build-server
domain=blocks.pfg.pw
image="$(sed -n 's|.*"container-image": "\(docker://[^"]*\)".*|\1|p' buck/tools/defs.bzl)"
```

Run them from a checkout of the repository.

1. **Install the packages.**
   ```
   sudo apt-get install -y busybox-static uidmap caddy skopeo umoci
   ```

2. **Add the user** and give it 65536 subordinate group IDs, starting at any
   range that `/etc/subgid` doesn't already use:
   ```
   sudo useradd --system --home-dir "$server" --create-home --shell /usr/sbin/nologin be3-build
   sudo usermod --add-subgids 165536-231071 be3-build
   ```

3. **Allow user namespaces.** Skip this step if
   `/proc/sys/kernel/apparmor_restrict_unprivileged_userns` is 0 or missing,
   and use `/usr/bin/unshare` in `run-action` below instead.
   ```
   sudo install -D -m 0755 /usr/bin/unshare /usr/local/libexec/be3-build-server/unshare
   sudo tee /etc/apparmor.d/be3-build-server << 'EOF'
   abi <abi/4.0>,
   include <tunables/global>

   profile be3-build-server /usr/local/libexec/be3-build-server/unshare flags=(unconfined) {
     userns,
   }
   EOF
   sudo apparmor_parser -r /etc/apparmor.d/be3-build-server
   ```

4. **Install NativeLink.** Check the archive's SHA-256 against the one shown
   here before extracting it.
   ```
   curl -fLO https://github.com/TraceMachina/nativelink/releases/download/v1.7.3/nativelink-1.7.3-x86_64-unknown-linux-musl.tar.gz
   echo '5b35a3296d91c12eac6fd4253f52c8890dd0e3ea079ac3694aa8f2c524c80f0b  nativelink-1.7.3-x86_64-unknown-linux-musl.tar.gz' | sha256sum -c
   sudo mkdir -p "$server/nativelink-1.7.3"
   sudo tar -xzf nativelink-1.7.3-x86_64-unknown-linux-musl.tar.gz -C "$server/nativelink-1.7.3" --no-same-owner nativelink
   ```

5. **Unpack the workers' image** into `$server/rootfs`, as `be3-build`.
   `rootfs` must contain `$server/work` and a working `resolv.conf`, because
   `action.sh` mounts the work directory there.
   ```
   sudo -u be3-build sh -c "cd '$server' && skopeo copy --override-arch amd64 --override-os linux '$image' oci:image:build && umoci raw unpack --rootless --image image:build rootfs && rm -rf image"
   sudo -u be3-build cp -L /etc/resolv.conf "$server/rootfs/etc/resolv.conf"
   sudo -u be3-build mkdir -p "$server/rootfs$server/work" "$server/work"
   ```

6. **Write `$server/action.sh`**, which runs one action inside the image:
   ```
   sudo tee "$server/action.sh" << 'EOF'
   set -e
   rootfs="$1"
   work="$2"
   shift 2

   mount --rbind /dev "$rootfs/dev"
   mount --rbind /sys "$rootfs/sys"
   mount -t proc proc "$rootfs/proc"
   mount -t tmpfs tmpfs "$rootfs/tmp"
   mount -t tmpfs tmpfs "$rootfs/root"
   mount --bind "$work" "$rootfs$work"

   export PATH="${PATH:-/usr/local/sbin:/usr/local/bin:/usr/sbin:/usr/bin:/sbin:/bin}"
   export HOME="${HOME:-/root}"
   export TAR_OPTIONS="--no-same-owner${TAR_OPTIONS:+ $TAR_OPTIONS}"

   exec chroot "$rootfs" /bin/sh -c '
       cd "$0" || exit
       for name in LD_LIBRARY_PATH LD_PRELOAD LD_AUDIT; do
           eval "if [ -n \"\${BE3_ACTION_$name+x}\" ]; then export $name=\"\$BE3_ACTION_$name\"; unset BE3_ACTION_$name; fi"
       done
       exec "$@"
   ' "$PWD" "$@"
   EOF
   ```
   NativeLink gives an action only the environment its command names, so
   `PATH` and `HOME` default to the image's. Only root and the subordinate
   groups exist in the namespace, so tar is told not to restore other owners.
   `dpkg-deb` asks for owners anyway, and its packages belong to root.

7. **Write `$server/run-action`**, NativeLink's entrypoint for every action:
   ```
   sudo tee "$server/run-action" << 'EOF'
   #!/bin/busybox sh
   for name in $(echo "${BE3_BUILD_SERVER_SECRETS:-}" | tr ',' ' '); do
       if [ -r "/etc/be3-build-server/secrets/$name" ]; then
           export "$name=$(cat "/etc/be3-build-server/secrets/$name")"
       fi
   done
   unset BE3_BUILD_SERVER_SECRETS
   for name in LD_LIBRARY_PATH LD_PRELOAD LD_AUDIT; do
       eval "if [ -n \"\${$name+x}\" ]; then export BE3_ACTION_$name=\"\$$name\"; unset $name; fi"
   done
   exec /usr/local/libexec/be3-build-server/unshare --map-user=0 --map-group=0 --map-groups=auto --mount --pid --fork --kill-child -- \
       /bin/sh /var/lib/be3-build-server/action.sh /var/lib/be3-build-server/rootfs /var/lib/be3-build-server/work "$@"
   EOF
   sudo chmod 755 "$server/run-action"
   ```

8. **Write `$server/nativelink.json5`.** Set the CAS's `max_bytes` to about
   60% of the disk's size. `max_bytes` bounds only `cas`; the rest of the disk
   is for `cas.exec` (see Disk space below), the image, the action cache and
   actions' work directories. Set `max_inflight_tasks` to the machine's core
   count.
   ```
   sudo tee "$server/nativelink.json5" << 'EOF'
   {
     stores: [
       {
         name: "CAS",
         fast_slow: {
           fast: {
             filesystem: {
               content_path: "/var/lib/be3-build-server/cas",
               temp_path: "/var/lib/be3-build-server/cas-tmp",
               eviction_policy: { max_bytes: 60000000000 },
             },
           },
           slow: { noop: {} },
         },
       },
       {
         name: "AC",
         completeness_checking: {
           backend: {
             filesystem: {
               content_path: "/var/lib/be3-build-server/ac",
               temp_path: "/var/lib/be3-build-server/ac-tmp",
               eviction_policy: { max_bytes: 1000000000 },
             },
           },
           cas_store: { ref_store: { name: "CAS" } },
         },
       },
     ],
     schedulers: [
       {
         name: "MAIN",
         simple: {
           supported_platform_properties: {
             OSFamily: "ignore",
             "container-image": "ignore",
           },
         },
       },
     ],
     workers: [
       {
         local: {
           worker_api_endpoint: { uri: "grpc://127.0.0.1:50062" },
           cas_fast_slow_store: "CAS",
           upload_action_result: { ac_store: "AC" },
           work_directory: "/var/lib/be3-build-server/work",
           entrypoint: "/var/lib/be3-build-server/run-action",
           max_inflight_tasks: 6,
           platform_properties: {},
         },
       },
     ],
     servers: [
       {
         name: "public",
         listener: { http: { socket_address: "127.0.0.1:50052" } },
         services: {
           cas: [{ instance_name: "", cas_store: "CAS" }],
           ac: [{ instance_name: "", ac_store: "AC" }],
           execution: [{ instance_name: "", cas_store: "CAS", scheduler: "MAIN" }],
           capabilities: [{ instance_name: "", remote_execution: { scheduler: "MAIN" } }],
           bytestream: [{ instance_name: "", cas_store: "CAS" }],
         },
       },
       {
         name: "workers",
         listener: { http: { socket_address: "127.0.0.1:50062" } },
         services: { worker_api: { scheduler: "MAIN" }, health: {} },
       },
     ],
     global: { max_open_files: 24576 },
   }
   EOF
   ```

9. **Start the service.**
   ```
   sudo tee /etc/systemd/system/be3-build-server.service << 'EOF'
   [Unit]
   Description=be3's remote execution server (guides/build_server.md)
   After=network.target
   RequiresMountsFor=/var/lib/be3-build-server

   [Service]
   User=be3-build
   ExecStart=/var/lib/be3-build-server/nativelink-1.7.3/nativelink /var/lib/be3-build-server/nativelink.json5
   Restart=on-failure
   LimitNOFILE=65536

   [Install]
   WantedBy=multi-user.target
   EOF
   sudo systemctl daemon-reload
   sudo systemctl enable --now be3-build-server
   ```

10. **Make the key and the secrets directory.**
    ```
    sudo install -d -m 0750 -o root -g be3-build /etc/be3-build-server /etc/be3-build-server/secrets
    head -c 32 /dev/urandom | od -An -tx1 | tr -d ' \n' | sudo tee /etc/be3-build-server/key > /dev/null
    sudo chmod 600 /etc/be3-build-server/key
    ```
    Copy CI's Android keystore from the old server to
    `/etc/be3-build-server/secrets/ANDROID_DEBUG_KEYSTORE_BASE64`, with mode
    0640, owned by `root:be3-build`. If the old one is lost, make a new
    keystore and store it in base64 there instead (see Secrets above).

11. **Put Caddy in front.** Write `/etc/caddy/Caddyfile` with this site, with
    `KEY` replaced by the key, mode 0640, group `caddy`. Then run
    `sudo systemctl enable caddy` and `sudo systemctl restart caddy`.
    ```
    DOMAIN {
    	@authorized header Authorization "Bearer KEY"
    	handle @authorized {
    		reverse_proxy h2c://127.0.0.1:50052 {
    			flush_interval -1
    		}
    	}
    	handle {
    		header Content-Type application/grpc
    		header Grpc-Status 16
    		header Grpc-Message "This build server needs its key: guides/build_server.md"
    		respond 200
    	}
    }
    ```

12. **If the domain is new,** change it in three places:
    - the `[buck2_re_client]` addresses in `.buckconfig`
    - the example host in `scripts/internal/re-relay/main.go`
    - this guide

13. **Hand out the key** to every machine that builds, and to CI as the secret
    `BE3_BUILD_SERVER_KEY`.

The cache starts empty, so the first build of everything takes a while.
`//:check` took about 16 minutes on six cores.

## Looking after it

- **Logs:** `journalctl -u be3-build-server` for the server,
  `journalctl -u caddy` for Caddy.
- **After any file in `$server` changes:** restart the service.
- **When `worker_properties` changes the image:** stop the service, then
  delete `rootfs`. It holds files with other groups, so delete it as
  namespace root:
  `sudo -u be3-build /usr/local/libexec/be3-build-server/unshare --map-user=0 --map-group=0 --map-groups=auto rm -rf "$server/rootfs"`.
  Then repeat step 5 and start the service again.
- **New key:** write a new `/etc/be3-build-server/key`, update the Caddyfile,
  restart Caddy, and hand the new key out.
- **Empty the cache:** stop the service, delete `cas`, `ac` and `work` in
  `$server`, and start the service again. Every client must also run
  `./scripts/buck kill`, because buck2 remembers which blobs it uploaded.
- **Disk space.** `max_bytes` does not bound the disk. NativeLink also keeps
  a second copy of every blob an action used as an executable in `cas.exec`,
  which no limit covers; it grew to 18 GB beside an 80 GB CAS and filled a
  100 GB disk, and actions failed with `No space left on device`. NativeLink
  empties `cas.exec` when it starts, so restarting the service frees it. To
  shrink the cache, lower `max_bytes` and restart; NativeLink evicts down to
  it as it starts. If the cache has a disk of its own, give `be3-build` the
  blocks ext4 reserves for root: `sudo tune2fs -m 0 DEVICE`.
- **What the key gives away.** Anyone with the key can run any command on the
  machine as `be3-build`, so they can also read its secrets.

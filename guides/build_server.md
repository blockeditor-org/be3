# The build server

Every buck2 action runs on a remote execution server. `BE3_BUILD_SERVER` picks which, or else
the one saved in `~/.config/be3/build-server`; at a terminal with neither, `./scripts/buck`
asks and saves the answer (delete the file to be asked again). Otherwise it is `nativelink`, the
NativeLink server on boat below. CI sets `BE3_BUILD_SERVER=nativelink` in .github/workflows/ci.yml.

`api.be3.pfg.pw` was much slower than Namespace was, so verify builds only what its tests and
lints need: Linux, the apps' wasm, and beui's HTML core only where a test runs on it. Nothing is
compile-checked for any other platform: not the web's be-host, the libraries planned for the
HTML core, Windows, macOS or Android. Check one by hand when a change is likely to break it, such
as `./scripts/buck build '//crates/be-host:be-host[check]' --target-platforms
//buck/platforms:wasi` (or `:windows_x86_64`; a crate on the HTML core, such as
`//crates/beui-html:beui-html[check]`, with `:linux_x86_64_html`). `//buck/dev/broad.bxl:main` checks them all
(guides/buck2.md, Traps). The bootstrap's six-platform release is built only
when CI is given `--bootstrap`, which it is only on a pull request that changes what the
release is built from (guides/bootstrap.md).

| `BE3_BUILD_SERVER` | Server | Key | In a Claude Code cloud session |
|---|---|---|---|
| `nativelink` (default, and CI's) | our NativeLink server on boat, through a WebSocket tunnel | its address `BE3_NL_URL`, and a boat key `BE3_NL_KEY` to wake it | works through the proxy once the address is set; needs Go |
| `api.be3.pfg.pw` | our NativeLink server | gateway header | works with no setup |
| `blocks.pfg.pw` | our NativeLink build VM, behind its own gateway over WireGuard | its gateway's header | works once the proxy adds the gateway header for it |
| `namespace` | Namespace's remote execution, no longer reachable | gateway header, to its waker | not working |
| `buildbuddy` | `remote.buildbuddy.io` | `x-buildbuddy-api-key` | not working |
| `local` | a NativeLink server of your own at `host:port` | none, no TLS | too small to host |

`local`'s address is `BE3_BUILD_SERVER_ADDRESS` or the saved file's second line; it is dialed
directly, not through the relay. The servers share nothing, so each has its own cache.
`./scripts/buck` writes the chosen server's addresses and header into
`target/build-server.buckconfig`, which `.buckconfig` includes if it exists (`<?file:...>`), and
restarts buck2's daemon when that file changes, since buck2 reads its remote execution settings
only when the daemon starts. `.buckconfig.local` is read after it and is only ever a person's:
`./scripts/buck` never writes it, so a value there overrides the generated one. Every
server runs actions in the one image `worker_properties` (buck/tools/defs.bzl) names; nothing in
the rules depends on which server it is. With no server at all, buck2 builds on this machine,
`./scripts/verify` included (guides/running/local_buck2.md): when the configuration in effect
names the local execution platforms or turns remote execution off, `./scripts/buck` asks no
server or waker for anything.

**Keys.** Namespace's waker and `api.be3.pfg.pw` sit behind the API gateway, so
`./scripts/buck` sends the `Be3-Gateway-Authorization` header, as `./scripts/be` does
(guides/tracker.md): from `BE3_AUTHORIZATION`, `.gateway-authorization` at the checkout's root,
or `~/.config/be3/gateway-authorization`, asking at a terminal when none works.
`blocks.pfg.pw` has its own gateway with a different key, sent in the same header: from
`BE3_BUILD_SERVER_KEY`, `.build-server-key.blocks.pfg.pw` or
`~/.config/be3/build-server-key.blocks.pfg.pw`. CI sets `BE3_AUTHORIZATION` from the
`BE3_TRACKER_AUTHORIZATION` secret; in an agent's cloud session the proxy adds the header
itself for the hosts it has a rule for. BuildBuddy's key is `BE3_BUILD_SERVER_KEY`, `.build-server-key.buildbuddy` or
`~/.config/be3/build-server-key.buildbuddy`; a cloud session has none.

## The NativeLink server on boat (`nativelink`)

Everything it runs is in scripts/internal/nativelink/ (none of it is a secret). A small `nl-scheduler` VM runs
NativeLink (CAS, action cache, scheduler), `nl-controller.py` and `nl-tunnel.py`; boat VMs named `nl-w-*` in the
`nl-worker` environment run the actions, each NativeLink's worker inside the image `worker_properties` names. The
scheduler and the workers meet in a WireGuard network, 10.100.0.0/24: the scheduler is 10.100.0.1 and serves buck2 on
port 50052.

- **A client reaches it through a tunnel**, so it needs no root, no WireGuard and no IPv6, and it works through an
  HTTPS proxy. Boat's hosted-port proxy puts the scheduler's port 8099 behind a token and carries WebSockets but
  blocks gRPC, so `nl-tunnel.py` on the scheduler accepts a WebSocket and connects it to NativeLink, and
  scripts/internal/re-relay (`ensure-tunnel`, tunnel.go) listens on 127.0.0.1 here and carries each TCP connection
  buck2 opens inside one WebSocket. buck2's HTTP/2 goes through untouched. The bootstrap
  (`crates/be-tool-bootstrap/src/nativelink.rs`) points buck2 at that local address, and builds the relay with Go
  (1.24 or newer; Go fetches the toolchain itself when it is older).
- **Two secrets**, each an environment variable, a file in ~/.config/be3, or a file at the checkout's root.
  `BE3_NL_URL` (`nl-url`, `.nl-url`) is the hosted port's address with its token, as `host url 8099` prints it on the
  scheduler: it is what lets a build use the server, so treat it as the gateway's header was. `BE3_NL_KEY` (`nl-key`,
  `.nl-key`) is a boat key limited to the `nl-scheduler` environment with the actions `sandbox.read` and
  `sandbox.resume`; it only wakes the scheduler when boat has stopped it, so a running scheduler needs the address
  alone. `BE3_NL_SCHEDULER` names another scheduler to wake by name prefix.
- **The controller** notices a build (a connection from the tunnel on port 50052), keeps 4 workers alive while it runs
  by extending their 30-minute lifetime, creates replacements, and stops the scheduler after 4 idle hours. Boat
  places a new VM at random and about half cannot reach the scheduler (below), so it creates twice as many workers as
  it needs and keeps the first that complete a WireGuard handshake. Its own boat key is limited to the `nl-worker`
  environment (create, read, update, delete-own).
- **Storage**: the workers read objects of 16 KB and up straight from the Bunny zone `be-nativelink-s3` with its
  read-only key (the `nl-worker` environment's secret file `worker-ro.env`); the scheduler keeps small objects and the
  action cache in /tmp, persisted as one packed Bunny object every 5 minutes and restored on boot, and writes
  everything else to Bunny with the full-access key (the `nl-scheduler` environment's `nl-bunny.env`).
- **Speed**: a one-file change to be-lib-geometry took 250-350 s with a fresh worker fleet and 190-230 s with a warm
  one.

Things that are not obvious:

- **Boat's network** blackholes about half of the VM pairs, both ways and for good, with no pattern we found. That
  does not touch clients, whose traffic goes through boat's proxy, but it does touch the scheduler and its workers,
  which is why workers are over-provisioned. A scheduler that boat resumed as a new machine can land where its workers
  cannot reach it; stopping and resuming it again, or forking it, picks another place.
- **Boat's start limits** (60 an hour, 200 a day on the $20 plan; `boat limits`) count every VM the controller creates,
  including the ones it deletes at once. A cold fleet is up to 8 starts.
- **Exposing the tunnel is one manual step** on the scheduler, `host 8099 --localhost`; the address keeps working
  across stop and resume, but a new scheduler has a new one.
- **After a restore** (boat stopped the VM and started a new machine) enabled services start before the disk has its
  files, so the units retry (`wg-quick-retry.conf`) instead of failing for good.

Setting the VMs up again: `scheduler/setup-scheduler.sh` on a VM made in the `nl-scheduler` environment, then the
controller (`nl-controller.service`, its worker files in /opt/nl/worker, its boat key logged in under
/var/lib/nl-controller/sk-home); each file's first lines say what it does.

### Starting over

The scheduler keeps nothing that cannot be rebuilt: the cache is in Bunny, and its small objects come back from the
packed object on boot. To make a new one, with the Bunny zone `be-nativelink-s3` (S3 enabled when the zone was made;
it cannot be turned on later) and a boat login that can create sandboxes:

1. **Environments** (once): `boat env new nl-worker` and `boat env set nl-worker --github false --secrets true
   --boat-credentials false --agents-credentials false`, then `boat env set-file nl-worker worker-ro.env --from FILE`
   where FILE holds `BUNNY_S3_RO_SECRET=` and the zone's read-only key. `boat env new nl-scheduler`,
   `boat env set nl-scheduler --github false --secrets true --boat-credentials true --agents-credentials false`
   (the scheduler stops itself with its own token), then `boat env set-file nl-scheduler nl-bunny.env --from FILE` where
   FILE holds `BUNNY_S3_KEY_ID=be-nativelink-s3` and `BUNNY_S3_SECRET=` with the full-access key.
2. **Keys**: the controller's, limited to `nl-worker` with `sandbox.create,sandbox.read,sandbox.update,sandbox.delete-own`
   and `account.read` (without it the controller counts its own starts instead of reading `boat limits`); and the wake
   key `BE3_NL_KEY`, limited to `nl-scheduler` with `sandbox.read,sandbox.resume`.
3. `bash scripts/internal/nativelink/new-scheduler.sh NAME CONTROLLER_KEY_FILE` makes the VM, installs NativeLink,
   the bridge and the controller, and prints what is left.
4. **Expose the tunnel**: `host 8099 --localhost`, then `host url 8099` prints the address with its token; store it as
   `BE3_NL_URL`. A new scheduler has a new address.
5. Delete the old scheduler (`boat delete`), and the clients' secrets are the only thing that changed.

### What the controller spends

Every VM it creates is a start. Past the plan's included starts a start costs $0.03 (up to 1.5 times the included
number), $0.05 (up to 2 times) or $0.07 (up to 6 times, 3 times a minute), and fails above that; the price is the highest
of the per-minute, per-hour and per-day ones (docs.boat.dev/limits#extra-starts). A VM created and deleted at once (one
that could not reach the scheduler) is a start too, so the controller learns what share of new VMs reach the scheduler
and creates only as many as that share needs. Its default is never to pay: it leaves 40 starts of the day and 10 of the
hour for everybody else's VMs and stops creating when the rest is used. `NL_MAX_START_PRICE` (the dearest start it will
buy) and `NL_EXTRA_START_BUDGET` (dollars a month) let it buy more; with `account.read` it asks boat what the next
start costs, without it it can only count its own. Between `NL_WORKERS_BASE` (4) and `NL_WORKERS_CAP` (12) workers it
adds some when they average 75% busy and removes the least busy after five minutes under 35%.

## When a build keeps failing

Behind an HTTPS proxy, as in a cloud session, buck2 reaches the server through
`scripts/internal/re-relay` (guides/buck2.md), whose errors are in `target/re-relay.log`.

- **`unexpected EOF`, `context canceled`, a connection reset**: a dropped call. buck2 fails with
  an infrastructure error and `./scripts/buck` retries the command (three runs in all), finding
  what finished cached. Failing every time means the server is down or overloaded. The proxy
  also cuts every call after five minutes; the relay then repeats the `Execute`, which joins the
  running action (`broke off ..., making it again` in its log), and buck2 sees nothing.
- **`the proxy answered 401 Unauthorized`**: the session has no key for that server
  (`credentials missing`) or a refused one (`invalid token`).
- **A test's output**: `./scripts/test` prints it from the `--event-log` buck2 test wrote to
  `target/test-events.json-lines`; when nothing ran, what buck2 said is in `target/test.log`.
- **A test that hangs**: a test run is stopped after `TEST_TIME_LIMIT` (buck/dev/test.bzl);
  libtest's `has been running for over 60 seconds` line names it. Build the binary with
  `./scripts/buck build //crates/X:test --show-full-output` and run it here with a name filter
  and `CARGO_MANIFEST_DIR=$PWD/crates/X` (painting tests need it). A timing-dependent hang can
  take dozens of runs; run several copies at once in a loop, and
  `gdb -p PID -batch -ex 'thread apply all bt'` shows what a hung one waits on.

## Namespace

Namespace's remote execution (https://namespace.so/docs/bazel/execution) runs on a cluster in
the repository's Namespace workspace: an executor (`reapih-…`) and storage for the action cache
and CAS (`storageh-…`).

Namespace shuts the cluster down when idle, and its hosts then answer 404, so no host is checked
in. Every run, `./scripts/buck` asks the waker at `https://api.be3.pfg.pw/build/namespace`
(`NAMESPACE_URL` in crates/be-tool-bootstrap/src/server.rs) for the buck2 configuration, and
says it is waking the cluster when the answer takes over two seconds. The waker is the build
helper's (below); its doc comments say when it runs `nsc` again. Builds never run `nsc`.

Every setup must come from that one host with the same pinned `nsc`, arguments and `--key`:
Namespace starts a new cluster beside the old one when any of them differ, or when two setups
race. Destroy the stale one in the dashboard, since its workers count against the workspace's
32 vCPU limit. Changing the `nsc` pin can do the same.

**Hosting the waker**: start the build helper with `--token ns-token.json --nsc ./nsc`.
`sh scripts/internal/bootstrap.sh install-nsc linux_amd64` downloads the pinned `nsc`; the token
comes from `nsc reapi create-token --no_expiry --token ns-token.json` with a Namespace login,
and is revoked at https://cloud.namespace.so/user/sessions.

**The workers' image** is `container-image` in `worker_properties` (buck/tools/defs.bzl), so an
action sees the same system everywhere. Namespace runs only images in its registry, `nscr.io`,
pinned by digest and optimized; any other fails with `the image is not optimized for remote
execution`, and its default image cannot run the toolchain. A new image:

```
nsc base-image upload docker.io/library/IMAGE@sha256:DIGEST be3-worker:TAG
nsc base-image optimize --image_ref nscr.io/WORKSPACE/be3-worker@sha256:DIGEST
```

**How actions run**: as root in the image, in a fresh directory, without per-action sandboxing,
so an action also sees the worker's environment, `NSC_TOKEN_FILE` among it. The worker replaces
an action's `LD_LIBRARY_PATH`; a test that needs its own gets it from crates/be-tool-run-test
(`--library-path`, from the `LD_LIBRARY_PATH` in its `be_test`'s env).

## The build helper

crates/be-tool-build-helper builds named products of a commit on request, on one long-lived host
whose `buck-out` persists, and uploads them to the be3-ci Bunny zone, so devices download from
the CDN and the build server's outputs are downloaded once. be-launcher asks it for APKs and
native builds. Its doc comments (`Host`, `Builds`, `Waker`) cover the rules.

- `POST /build/products/PRODUCT/COMMIT` asks for a build and answers its status as JSON
  (`queued` with `position`, `running`, `succeeded` with `manifest`, `failed` with `error`); `GET`
  reads it without asking. `GET .../log?from=N` streams the build's log from byte N.
- Products (crates/be-tool-build-helper/src/products.rs): `android` and `launcher-android`
  (`:android-dist`, signed with CI's key), and `native-PLATFORM` and `beui-demo-PLATFORM`, release
  builds for the six Linux, macOS and Windows platforms.
- Only commits on a branch of the repository are built (422 otherwise), so a fork's code never
  runs on workers that see credentials.

### Hosting it

It checks no credentials, so it listens on localhost behind the API host's Caddy, which routes
`/build/*` to it after the gateway's check (guides/hosting.md). Build it with
`./scripts/buck build //crates/be-tool-build-helper:bin` and run it as a service with `git`,
`curl` and what `./scripts/buck` needs:

```
BUNNY_STORAGE_KEY=... be-tool-build-helper --checkout /srv/build-helper/checkout \
  --state /srv/build-helper/state --keystore /srv/build-helper/ci-keystore.base64 \
  [--addr 127.0.0.1:9091] [--public https://be3-ci.b-cdn.net] [--max-buck-out-gb 40] \
  [--token ns-token.json --nsc ./nsc]
```

- `--checkout` is a clone used only by the helper, which cleans it before each build; the build
  server's key goes in its root or the service's environment.
- `--state` holds logs, manifests, upload records and signed APKs.
- `--keystore` is CI's Android keystore (JKS/PKCS12 or base64 of one, password `android`). Keep it
  outside the checkout. Without it Android builds fail; replacing it means devices must uninstall
  before updating (guides/running/android.md).
- Without `BUNNY_STORAGE_KEY` nothing is uploaded; the log says what would be.
- Past `--max-buck-out-gb` after a build, it runs `./scripts/buck clean`.

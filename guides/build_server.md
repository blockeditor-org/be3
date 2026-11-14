# The build server

Every buck2 action runs on a remote execution server or on this machine. `BE3_BUILD_SERVER`
picks where, or else the first line of `~/.config/be3/build-server`; at a terminal with neither,
`./scripts/buck` asks and saves the answer (delete the file to be asked again). Otherwise it is
`nl.be3.pfg.pw`, the NativeLink server below. CI sets `BE3_BUILD_SERVER=nl.be3.pfg.pw` in
.github/workflows/ci.yml.

The build server is slow enough that verify builds only what its tests and lints need: Linux,
the apps' wasm, and beui's HTML core only where a test runs on it. Nothing is compile-checked
for any other platform: not the web's be-host, the libraries planned for the HTML core,
Windows, macOS or Android. Check one by hand when a change is likely to break it, such as
`./scripts/buck build '//crates/be-host:be-host[check]' --target-platforms
//buck/platforms:wasi` (or `:windows_x86_64`; a crate on the HTML core, such as
`//crates/beui-html:beui-html[check]`, with `:linux_x86_64_html`). `//buck/dev/broad.bxl:main` checks them all
(guides/buck2.md, Traps). The bootstrap's six-platform release is built only
when CI is given `--bootstrap`, which it is only on a pull request that changes what the
release is built from (guides/bootstrap.md).

| `BE3_BUILD_SERVER` | Where | Key |
|---|---|---|
| `nl.be3.pfg.pw` (default, and CI's) | our NativeLink server on one Hetzner machine, behind Caddy (below) | Caddy's token, `BE3_NL_TOKEN` |
| `custom` | a NativeLink server of your own at `host:port`, dialed directly, without TLS | none |
| `local` | this machine, with no build server (guides/running/local_buck2.md) | none |

`custom`'s address is `BE3_BUILD_SERVER_ADDRESS` or the saved file's second line. Each server
has its own cache. `./scripts/buck` writes the server's address and header, or `local`'s
settings, into `target/build-server.buckconfig`, which `.buckconfig` includes if it exists
(`<?file:...>`), and restarts buck2's daemon when that file changes, since buck2 reads its
remote execution settings only when the daemon starts.

`.buckconfig.local` is read after it and is only ever a person's, handwritten: `./scripts/buck`
never writes it. When it exists, buck2 builds where it says, on the server it names or on this
machine: `./scripts/buck` asks for no server or key and removes `target/build-server.buckconfig`,
so its addresses and header are not mixed into the person's. Every server runs actions in the one
image `worker_properties` (buck/tools/defs.bzl) names; nothing in the rules depends on which
server it is.

## nl.be3.pfg.pw

One Hetzner CPX62 (16 shared AMD EPYC Genoa vCPUs, 32 GB, 601 GB of local disk, in Helsinki, €111 a month) runs
NativeLink's cache, scheduler and worker as one process, inside the image `worker_properties` names, so the worker
hardlinks inputs from the same store clients upload to and nothing crosses a network between them. Everything it runs
is in scripts/internal/nativelink/ (none of it is a secret); `bash setup.sh nl.be3.pfg.pw`, run as root on a new
Ubuntu 24.04 server, installs it, and running it again updates it.

- **The front door** is Caddy on port 443 (`Caddyfile`, `nl-caddy.service`), with a Let's Encrypt certificate it gets
  itself for nl.be3.pfg.pw. It answers 401 to any request without `Authorization: Bearer TOKEN` and forwards the rest
  to NativeLink on 127.0.0.1 over cleartext HTTP/2. The token is `NL_TUNNEL_TOKEN` in `/etc/nl-caddy.env` on the
  server, which `setup.sh` makes once.
- **The key** is that token: `BE3_NL_TOKEN`, `.nl-token` at the checkout's root or `~/.config/be3/nl-token`, asked
  for at a terminal. CI has it as the secret `BE3_NL_TOKEN`. buck2 reaches the server directly over TLS; behind an HTTPS proxy, through the relay.
- **Behind a proxy, through the tunnel.** A cloud session's proxy speaks only HTTP/1.1, handles about 240 requests a
  second however many connections carry them, and adds about 80 ms to each request on its way up; a cache-warm
  `./scripts/verify` makes about 24,000 calls, a GetActionResult and then a BatchReadBlobs for each cached action, one
  after the other. With each call its own request that took about 1m45 from Iowa. So for nl.be3.pfg.pw the relay runs
  with `-tunnel` (scripts/internal/re-relay/tunnel.go): HTTP/2 inside one byte stream, carried up by one long chunked
  POST and down by one long GET, both of which the proxy passes along as they come. Each is replaced every 4 minutes,
  under the proxy's five-minute cut, from where the other side got to, and the relay asks `/__be3-tunnel/status` about
  once a second how far the server got, so that each side can forget what the other has. Caddy passes
  `/__be3-tunnel/*` to re-relay's `tunnel-server` (`nl-tunnel.service`), which joins each tunnel to NativeLink.
  `BE3_NL_TUNNEL=0` sends each call as its own request instead, to compare.
- **Measuring the way.** Every 30 s the relay writes each gRPC method's count and times, and each tunnel's bytes and
  requests, to `target/re-relay.log`; the tunnel-server logs the most bytes that queued behind each GET
  (`journalctl -u nl-tunnel`). `re-relay tunnel-probe -upstream nl.be3.pfg.pw` (the token in `BE3_NL_TOKEN` unless a
  proxy adds it) times each way through the proxy: POSTs up, lines streamed 100 ms apart down, which arrive in bunches
  when the proxy holds a stream back, and one streaming POST, whose lines arrive late when it collects a body first.
- **In a cloud session** the proxy adds `Authorization: Bearer TOKEN` for the host nl.be3.pfg.pw as a network secret,
  and the session's environment sets `BE3_BUILD_SERVER=nl.be3.pfg.pw` and no key, so buck2 sends no header.
- **Storage** is the server's own disk, which survives a reboot: the CAS keeps up to 270 GB
  (`/var/lib/nativelink/cas`) and the action cache 2 GB, each evicting the least recently used. The CAS's limit counts
  only `cas/content`: NativeLink also keeps a mode-0555 copy of each blob an action takes as an executable in
  `cas/content.exec`, which the limit does not see and which can grow as large again, so the limit is under half the
  disk. Nothing is copied
  elsewhere: a lost server means a new one and a cold cache, whose first `./scripts/verify` took 30 minutes, most of it
  the client uploading toolchain archives. buck2 and the relay remember for hours which blobs the server holds, so
  after emptying the cache by hand restart every client's daemon (`./scripts/buck kill`).
- **One worker runs 16 actions**, one per vCPU. A rustc runs many threads, so a full queue keeps the load far above 16
  (up to 280), and a lone tail action, such as the last big compile or link, gets all 16 vCPUs.
- **CI waits behind everyone else.** Among actions waiting for a slot NativeLink runs the highest priority first, then
  the oldest. CI's builds send -1 and everything else 0 (`REMOTE_PRIORITY`, buck/platforms/profile.bzl), so a merge
  queue's actions never hold up a person's or an agent's. The priority is in the request, not the action, so it changes
  no cache key. NativeLink does not raise the priority of an action it is already running for CI when an agent asks
  for the same one, so that agent waits as CI would.
- **Only the merge queue's last entry is built.** Every merge queue run shares one concurrency group in
  `.github/workflows/ci.yml`, and the newest cancels the rest: its merge group holds every entry ahead of it. The
  queue is set not to require every entry's checks to pass, so when it passes, the entries ahead merge with it.
- **Speed**, measured when it was set up:
  - `./scripts/verify` with everything cached: 56 s.
  - A one-file change to be-lib-geometry, one build alone: 192 s.
  - 8 such builds at once, each a different change, from 3 client VMs: all passed, in 918-1414 s. The CPU was
    86% busy (61% user, 25% system) with no steal, memory peaked at 11 GB, and 50 GB of the cache was in use. It is
    CPU-bound: a second machine would be the way to more.
- **Cost**: €110.99 a month flat with its IPv4 address and 20 TB of traffic, whatever it builds.

### The dashboard

https://nl.be3.pfg.pw/ is a Grafana dashboard of the server and of every build (`scripts/internal/nativelink/dashboard`,
which `setup.sh` installs). Caddy lets a browser through when it holds the cookie `be3_dash=NL_DASH_TOKEN` (from
`/etc/nl-caddy.env`); without it, the only page is `/gate`, which asks for the token and sets the cookie.

- **The machine**: node_exporter's CPU by mode (steal is a shared vCPU given to another machine), pressure (the share of
  time something waited for a CPU, memory or the disk), memory and swap, disk and network.
- **The build server**: `nl-stats.py` counts the actions running, one directory each under `/var/lib/nativelink/run`,
  every 5 s, for node_exporter's textfile collector. NativeLink sends its own metrics over OTLP to 127.0.0.1:4317 (it
  always tries), where the OpenTelemetry Collector (`otel-collector.yaml`) keeps them for Prometheus, minus a series
  per action digest: queue depth and queue time, actions finished, each action's peak memory, CAS reads and bytes.
  Prometheus keeps a year of both (`/var/lib/nl-prometheus`).
- **Builds**: at its end, every `./scripts/verify` and `./scripts/ci` that built on a named server sends a record
  (crates/be-tool-verify's stats.rs, from buck2's event log) to `/__be3-stats/build` with the build token, which
  `nl-stats.py` keeps in `/var/lib/nl-stats/builds.db`: how long buck2 took, the actions that ran and were cached,
  their total queue time, the critical path, every test run and every other action over 10 s. Its kind is
  `ci-EVENT` in CI, `verify-cloud` behind a proxy, and `verify` otherwise; `target/verify-record.json` is the last one
  sent. Ask it anything with `sqlite3 /var/lib/nl-stats/builds.db` on the server.
- The dashboard is `build-server.json`; the UI cannot save changes to it, so change the file and run `setup.sh`.

## When a build keeps failing

Behind an HTTPS proxy, as in a cloud session, buck2 reaches nl.be3.pfg.pw through `scripts/internal/re-relay`
(guides/buck2.md), whose errors are in `target/re-relay.log`.

- **`unexpected EOF`, `context canceled`, a connection reset**: a dropped call. buck2 fails with
  an infrastructure error and `./scripts/buck` retries the command (three runs in all), finding
  what finished cached. Failing every time means the server is down or overloaded. The proxy
  also cuts every call after five minutes; the relay then repeats the `Execute`, which joins the
  running action (`broke off ..., making it again` in its log), and buck2 sees nothing.
- **`action input missing from CAS`** (`not found in either fast or slow store`): the server lost a blob this client
  remembers it holding (above): `./scripts/buck kill` and run it again. When it survives
  `./scripts/buck clean`, run `./scripts/buck kill; rm -rf ~/.buck/buckd buck-out`.
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

## The build helper

crates/be-tool-build-helper builds named products of a commit on request, on one long-lived host
whose `buck-out` persists, and uploads them to the be3-ci Bunny zone, so devices download from
the CDN and the build server's outputs are downloaded once. be-launcher asks it for APKs and
native builds. Its doc comments (`Host`, `Builds`) cover the rules.

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
  [--addr 127.0.0.1:9091] [--public https://be3-ci.b-cdn.net] [--max-buck-out-gb 40]
```

- `--checkout` is a clone used only by the helper, which cleans it before each build; the build
  server's token goes in its root as `.nl-token` or in the service's environment as `BE3_NL_TOKEN`.
- `--state` holds logs, manifests, upload records and signed APKs.
- `--keystore` is CI's Android keystore (JKS/PKCS12 or base64 of one, password `android`). Keep it
  outside the checkout. Without it Android builds fail; replacing it means devices must uninstall
  before updating (guides/running/android.md).
- Without `BUNNY_STORAGE_KEY` nothing is uploaded; the log says what would be.
- Past `--max-buck-out-gb` after a build, it runs `./scripts/buck clean`.

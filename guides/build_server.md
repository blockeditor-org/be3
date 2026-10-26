# The build server

Every buck2 action runs on a remote execution server. `BE3_BUILD_SERVER` picks
which, or else the one saved in `~/.config/be3/build-server`. Without either,
at a terminal, `./scripts/buck` asks which and saves the answer there (delete
it to be asked again); anything else builds on Namespace.

| `BE3_BUILD_SERVER` | Server | Key | Claude Code Cloud Session |
|---|---|---|---|
| `namespace` (the default, and CI's) | Namespace's remote execution | the API gateway's header, to its waker | Should work with 0 configuration |
| `api.be3.pfg.pw` | our own NativeLink server | `authorization: Bearer KEY` | Should work with 0 configuration |
| `buildbuddy` | BuildBuddy's `remote.buildbuddy.io` | `x-buildbuddy-api-key: KEY` | Currently not working |
| `local` | a NativeLink server of your own, at `host:port` | none, and no TLS | Not powerful enough to host |

`local`'s address, such as `127.0.0.1:50052`, is `BE3_BUILD_SERVER_ADDRESS`,
or else the second line of `~/.config/be3/build-server`, where the prompt
saves it. It is dialed directly, never through `scripts/internal/re-relay`.

The servers share nothing: each has its own cache and its own key.
`./scripts/buck` writes the one it picked into `.buckconfig.local`, with its
name as `be3.build_server`; `worker_properties` (`buck/tools/defs.bzl`) reads
that to pick the workers' image, since Namespace runs only its own copy of it.

Namespace's waker is behind the API gateway, so `namespace` sends the
gateway's `Be3-Gateway-Authorization` header, as `./scripts/be` does
(guides/tracker.md): `BE3_AUTHORIZATION`, or else `.gateway-authorization` at
the root of the checkout or `~/.config/be3/gateway-authorization`. When the
gateway refuses the request at a terminal, `./scripts/buck` asks for the key
until it is accepted and saves it to the last. CI sets the variable, and in an
agent's cloud session the proxy adds the header itself. A server other than `namespace` and
`local` takes its key from:

1. `BE3_BUILD_SERVER_KEY` in the environment
2. `.build-server-key.SERVER` at the root of the checkout
3. `~/.config/be3/build-server-key.SERVER`

where `SERVER` is the value of `BE3_BUILD_SERVER`. Without one, at a terminal,
`./scripts/buck` asks for it and saves it to the last.

In an agent's cloud session the proxy adds the key to requests for
`api.be3.pfg.pw`, and `BE3_BUILD_SERVER_KEY` is a
placeholder; there is no key for BuildBuddy there.

## When a build keeps failing

Behind an HTTPS proxy, as in an agent's cloud session, buck2 reaches the
server through `scripts/internal/re-relay`, whose errors are in
`target/re-relay.log`:

- **`unexpected EOF`, `context canceled`, a connection reset**: a call dropped
  on the way. buck2 fails the command with an infrastructure error (exit status
  2), and `./scripts/buck` tries such a command three times in all
  (`BE3_BUCK_ATTEMPTS`), each time finding what finished cached. A command that
  fails every time is a server that is down or overloaded.
  A test that hangs looks the same: the server cuts its action off after about
  five minutes, and it is reported as `re-relay: unexpected EOF`. When only a
  test command fails this way, build its binary with `./scripts/buck build
  //crates/X:test --show-full-output` and run the printed path here with a name
  filter. If it hangs here too, the test hangs, and `gdb -p PID -batch -ex
  'thread apply all bt'` shows what it waits on.
- **`the proxy answered 401 Unauthorized`**: the proxy has no key for the host
  (`credentials missing`) or refused the one it has (`invalid token`): the
  session has no key for that server.

Each checkout has relays of its own, on ports picked from its path
(guides/buck2.md), so two checkouts on one machine can build on different
servers at once.

## Namespace

Namespace's remote execution
(https://namespace.so/docs/bazel/execution) runs on a cluster that belongs to
the repository's Namespace workspace: an executor (`reapih-…`) and the storage
that holds the action cache and the CAS (`storageh-…`).

### Waking it

Namespace shuts the cluster down when it has been idle for a while, and its
hosts then answer 404. So no host is checked in: `./scripts/buck` asks
`https://api.be3.pfg.pw/build/namespace` (`NAMESPACE_URL` in
`crates/bootstrap/src/server.rs`) through the API gateway, and gets back
the buck2 configuration that names the executor, the storage and the
`x-nsc-ingress-auth` header that authenticates to them. It asks on every
run, keeps nothing, and writes the answer into `.buckconfig.local`. The waker
answers quickly from its own cache, so the only slow request is one that
finds the cluster asleep, and after two seconds of waiting `./scripts/buck`
says it is waking it. Builds never run `nsc`.

Behind that URL, the build helper (`crates/build-helper`, below), when given
`--token`, runs
`nsc reapi setup buck2 --token FILE --key buildserver`, which starts the
cluster if it is down, and answers with the configuration it writes. It hands
that answer out again until the executor stops answering a gRPC call (which it
makes with `curl`) or the answer is three and a half hours old, and only then
runs `nsc` again. Callers within a minute of a setup get its answer without
that check, so a crowd that finds the cluster asleep wakes it once. Every setup comes from that one host, with one
pinned `nsc` and the same arguments and `--key`: Namespace starts a new cluster
beside the old one when any of them differ, or when two setups race. The
dashboard shows both, and the stale one should be destroyed, since its workers
count against the workspace's 32 vCPU limit.

### Hosting the waker

The waker is the build helper's `GET /build/namespace`, served only when it is
started with `--token ns-token.json --nsc ./nsc` (hosting it is under "The
build helper" below).
`sh scripts/internal/bootstrap.sh install-nsc linux_amd64` downloads the
`nsc` pinned in `scripts/internal/pins` and
prints its path; the host also needs `curl`. The waker runs `nsc` with `NS_DO_NOT_UPDATE=1`, so `nsc`
changes only when that pin does. Changing the pin can change what a setup asks
for, which starts a new cluster: destroy the old one afterwards.

The token is made with a Namespace login, and revoked at
https://cloud.namespace.so/user/sessions:

```
nsc reapi create-token --no_expiry --token ns-token.json
```

When `HTTPS_PROXY` is set, `./scripts/buck` reaches the waker through it, and
buck2 reaches the cluster through `scripts/internal/re-relay`
(guides/buck2.md).

### The workers' image

Actions run in the image named by `container-image` in `worker_properties`
(`buck/tools/defs.bzl`), so an action sees the same system wherever it runs.
Namespace runs only images in its own registry, `nscr.io`, pinned by digest,
that it has optimized; any other is refused with `the image is not optimized
for remote execution`. Namespace's default image cannot run the toolchain.

A new image is copied in and optimized with a Namespace login:

```
nsc base-image upload docker.io/library/IMAGE@sha256:DIGEST be3-worker:TAG
nsc base-image optimize --image_ref nscr.io/WORKSPACE/be3-worker@sha256:DIGEST
```

`upload` prints the `nscr.io` reference with its digest, which is what
`optimize` and `worker_properties` take. A new digest needs optimizing once.

### How actions run

- **As root, in the image**, in a fresh directory, without per-action
  sandboxing: an action also sees the worker's own environment, among it
  `NSC_TOKEN_FILE`.
- **`LD_LIBRARY_PATH` is the worker's.** The worker replaces the one an action
  asks for. A test that needs its own (the ones that draw through lavapipe)
  carries it as `BE3_LD_LIBRARY_PATH`, and the shell that starts the test puts
  it back: `library_path_test` in `buck/cargo/defs.bzl`, which `cargo_test`
  makes for a test whose `env` names `LD_LIBRARY_PATH`, and `plugin_test_run`
  in `buck/wasm/defs.bzl`.

## The build helper

`crates/build-helper` builds named products of a commit on request, so that
the build server's outputs are downloaded once, by one long-lived host whose
`buck-out` persists, and devices download from the Bunny CDN instead. It is
what be-launcher asks for Android APKs and the native app.

- `POST /build/products/PRODUCT/COMMIT` asks for a build and answers its
  status as JSON: `queued` (with `position`, the builds ahead of it, the
  running one included), `running`, `succeeded` (with `manifest`, its URL) or
  `failed` (with `error`, one line). A published manifest answers `succeeded`
  without building; asking again for a queued or running build returns it,
  and asking again for a failed one builds it again. `GET` on the same path
  reads the status without asking.
- `GET /build/products/PRODUCT/COMMIT/log?from=N` is the build's log from byte
  N, everything its commands printed, with `Build-State` and
  `Build-Log-Length` headers. With nothing past N yet it waits up to 25
  seconds for more while the build is queued or running.
- Products are a fixed table: `android` and `launcher-android` (the
  `:android-dist` APK, signed with CI's keystore), and `native-PLATFORM`
  (`//crates/block-app:native`) and `beui-demo-PLATFORM`
  (`//crates/beui-demo:native`), built with `-m release --target-platforms
  root//buck/platforms:PLATFORM` for `linux_x86_64`, `linux_arm64`,
  `macos_x86_64`, `macos_arm64`, `windows_x86_64` and `windows_arm64`. The
  executables are named `block-app` and `beui-demo` (`.exe` on Windows).
- Every file is uploaded to the be3-ci storage zone as `files/SHA256`, once:
  the helper remembers what it uploaded and otherwise checks the CDN. Then the
  manifest, `builds/PRODUCT/COMMIT.json`, listing each file's path, SHA-256,
  size and whether it is executable, is uploaded last. Nothing deletes them.
- Only commits on a branch of the repository are built: the helper fetches
  `origin` and refuses (422) a commit that `git branch -r --contains` finds on
  no branch, so a fork's code never runs on the workers, which see credentials
  ("How actions run").
- One build runs at a time, in the order asked. Each checks out the commit in
  the helper's own checkout, runs `git clean -ffdx` keeping `buck-out/`,
  `target/` and the credential files at its root, and builds with
  `./scripts/buck`, so the build server is the service environment's
  (`BE3_BUILD_SERVER` and its key). An APK is built with
  `//buck/app:build-info.json` and `//crates/local-tools:release`, and signed
  by that local-tools `apk-sign` with `--keystore`, so the key never becomes an
  action input.

### Hosting the build helper

It checks no credentials, so it listens on localhost behind the API host's
Caddy, which routes `/build/*` to it after the gateway's check of
`Be3-Gateway-Authorization`, like every other API path (guides/hosting.md).
`./scripts/buck build //crates/build-helper:bin` builds it, and it runs as a
service with `git`, `curl` and what `./scripts/buck` needs on the host:

```
BUNNY_STORAGE_KEY=... build-helper --checkout /srv/build-helper/checkout \
  --state /srv/build-helper/state --keystore /srv/build-helper/ci-keystore.base64 \
  [--addr 127.0.0.1:9091] [--public https://be3-ci.b-cdn.net] [--max-buck-out-gb 40] \
  [--token ns-token.json --nsc ./nsc]
```

- `--checkout` is a clone of the repository whose `origin` is it on GitHub,
  used only by the helper; the build server's key goes in its root or the
  service's environment, as for any checkout.
- `--state` holds each build's log and final status (`logs/`), the manifests
  (`manifests/`), what was uploaded (`uploaded/`) and published
  (`published/`), and the signed APKs (`work/`). Its logs stay until deleted.
- `--keystore` is CI's Android keystore (below); keep it outside the checkout,
  which is cleaned. Without it Android builds fail.
- `BUNNY_STORAGE_KEY` is the storage zone's key, as CI's. Without it nothing
  is uploaded: the log says what would be, and the manifest is only written
  under `--state`.
- After a build, once `buck-out` is past `--max-buck-out-gb`, the helper runs
  `./scripts/buck clean`; everything is cached on the build server.

## CI's Android keystore

The build helper signs its `android` and `launcher-android` builds with CI's
keystore, the file its `--keystore` names, through `local-tools apk-sign`
after the build, so the key never becomes an action input. CI itself no
longer builds or signs an APK. Keep the same keystore so each build installs
as an update over the last. Keys are JKS/PKCS12 or base64 of one, with
password `android`. Key generation and release key handling are described in
guides/running/android.md.

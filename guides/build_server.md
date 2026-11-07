# The build server

Every buck2 action runs on a remote execution server. `BE3_BUILD_SERVER` picks which, or else
the one saved in `~/.config/be3/build-server`; at a terminal with neither, `./scripts/buck`
asks and saves the answer (delete the file to be asked again). Otherwise it is `api.be3.pfg.pw`.
CI sets `BE3_BUILD_SERVER=api.be3.pfg.pw` in .github/workflows/ci.yml.

`api.be3.pfg.pw` is much slower than Namespace was, so verify builds only what its tests and
lints need: Linux, the apps' wasm, and beui's HTML core only where a test runs on it. Nothing is
compile-checked for any other platform: not the web's be-host, the libraries planned for the
HTML core, Windows, macOS or Android. Check one by hand when a change is likely to break it, such
as `./scripts/buck build '//crates/be-host:be-host[check]' --target-platforms
//buck/platforms:wasi` (or `:windows_x86_64`, `:linux_x86_64_html`). The bootstrap's six-platform release is built only
when CI is given `--bootstrap`, which it is only on a pull request that changes what the
release is built from (guides/bootstrap.md).

| `BE3_BUILD_SERVER` | Server | Key | In a Claude Code cloud session |
|---|---|---|---|
| `api.be3.pfg.pw` (default, and CI's) | our NativeLink server | gateway header | works with no setup |
| `blocks.pfg.pw` | our NativeLink build VM, behind its own gateway over WireGuard | its gateway's header | works once the proxy adds the gateway header for it |
| `namespace` | Namespace's remote execution, no longer reachable | gateway header, to its waker | not working |
| `buildbuddy` | `remote.buildbuddy.io` | `x-buildbuddy-api-key` | not working |
| `local` | a NativeLink server of your own at `host:port` | none, no TLS | too small to host |

`local`'s address is `BE3_BUILD_SERVER_ADDRESS` or the saved file's second line; it is dialed
directly, not through the relay. The servers share nothing, so each has its own cache.
`./scripts/buck` writes the chosen server's addresses and header into `.buckconfig.local`. Every
server runs actions in the one image `worker_properties` (buck/tools/defs.bzl) names; nothing in
the rules depends on which server it is. With no server at all, buck2 builds on this machine
(guides/running/local_buck2.md).

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
- **A test that hangs**: a test run is stopped after `TEST_TIME_LIMIT` (buck/cargo/defs.bzl);
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
an action's `LD_LIBRARY_PATH`; tests that need their own carry it as `BE3_LD_LIBRARY_PATH`
(`library_path_test` in buck/cargo/defs.bzl, `app_test_run` in buck/wasm/defs.bzl).

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

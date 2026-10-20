# The build server

Every buck2 action runs on a remote execution server. `BE3_BUILD_SERVER` picks
which, or else the one saved in `~/.config/be3/build-server`. Without either,
at a terminal, `./scripts/buck` asks which and saves the answer there (delete
it to be asked again); anything else builds on Namespace.

| `BE3_BUILD_SERVER` | Server | Key | Claude Code Cloud Session |
|---|---|---|---|
| `namespace` (the default, and CI's) | Namespace's remote execution | `authorization: Bearer KEY` to its waker | Needs `BE3_NAMESPACE_KEY` |
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

A server other than `local` takes its key from:

1. `BE3_BUILD_SERVER_KEY` in the environment, or `BE3_NAMESPACE_KEY` for
   `namespace`, so that an environment can hold both
2. `.build-server-key.SERVER` at the root of the checkout
3. `~/.config/be3/build-server-key.SERVER`

where `SERVER` is the value of `BE3_BUILD_SERVER`. Without one, at a terminal,
`./scripts/buck` asks for it and saves it to the last.

In an agent's cloud session the proxy adds the key to requests for
`api.be3.pfg.pw`, and `BE3_BUILD_SERVER_KEY` is a
placeholder; there is no key for BuildBuddy there.

## When a build hangs or keeps failing

The servers fail in different ways:

- **Namespace** is the fastest, but now and then never finishes a build: an
  action sits at `[re_execute]` or queued for hours while nothing else moves.
  It does not recover on its own, so stop the command rather than wait.
- **`api.be3.pfg.pw`** always finishes, but runs slower.

To move off a server that hangs or keeps failing, run the command again with
another one, for that command or, exported, for the rest of the session:

```
BE3_BUILD_SERVER=api.be3.pfg.pw ./scripts/verify
```

You may also need to kill buck's daemon (`./scripts/buck kill`)

`./scripts/buck` rewrites `.buckconfig.local` and restarts the buck2 daemon on
every change of server. The servers share no cache, so the first build on a
server rebuilds everything it has not built before, which is most of the
workspace for a server that has not built this branch; switching pays off for
a build that is stuck, not for one that is merely slow.

Behind an HTTPS proxy, as in an agent's cloud session, buck2 reaches the
server through `scripts/internal/re-relay`, whose errors are in
`target/re-relay.log`:

- **`unexpected EOF`, `context canceled`, a connection reset**: a call dropped
  on the way. buck2 fails the command with an infrastructure error (exit status
  2), and `./scripts/buck` tries such a command three times in all
  (`BE3_BUCK_ATTEMPTS`), each time finding what finished cached. A command that
  fails every time is a server that is down or overloaded: switch.
- **`the proxy answered 401 Unauthorized`**: the proxy has no key for the host
  (`credentials missing`) or refused the one it has (`invalid token`): the
  session has no key for that server. Switch to a server the
  session has a key for.

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
`https://api.be3.pfg.pw/build/namespace` (`namespace_url` in
`scripts/internal/common.sh`) with `authorization: Bearer KEY`, and gets back
the buck2 configuration that names the executor, the storage and the
`x-nsc-ingress-auth` header that authenticates to them. It keeps that answer in
`target/namespace/`, writes it into `.buckconfig.local`, and asks again when
the executor stops answering, when the answer is three and a half hours old
(an answer lasts about four), or when the key changes. The answer's age counts
from its setup, which the waker sends as an `age` header. Builds never run
`nsc`.

Behind that URL, `namespace-waker` (`crates/namespace-waker`) runs
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

The waker checks no credentials, so it listens on localhost and the host's
Caddy checks the key, ahead of the API gateway's own rules:

```
api.be3.pfg.pw {
	@namespace path /build/namespace
	handle @namespace {
		@unauthorized not header Authorization "Bearer {$BE3_NAMESPACE_KEY}"
		respond @unauthorized 401
		reverse_proxy 127.0.0.1:9091
	}
	# the rest of the site
}
```

with the key in Caddy's environment, and the waker run as a service:

```
namespace-waker --token ns-token.json --nsc ./nsc --addr 127.0.0.1:9091
```

`./scripts/buck build //crates/namespace-waker:bin` builds it, and
`scripts/internal/install-nsc.sh linux_amd64` downloads the pinned `nsc` and
prints its path; the host also needs `curl`. The waker runs `nsc` with `NS_DO_NOT_UPDATE=1`, so `nsc`
changes only when that pin does. Changing the pin can change what a setup asks
for, which starts a new cluster: destroy the old one afterwards.

The token is made with a Namespace login, and revoked at
https://cloud.namespace.so/user/sessions:

```
nsc reapi create-token --no_expiry --token ns-token.json
```

### The key

- **CI:** `BE3_NAMESPACE_KEY` is the repository secret of that name.
- **An agent's cloud session:** `BE3_NAMESPACE_KEY` is set in the
  environment's variables. When `HTTPS_PROXY` is set, `./scripts/buck` reaches
  the waker through it, and buck2 reaches the cluster through
  `scripts/internal/re-relay` (guides/buck2.md).

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

## CI's Android keystore

`:android-dist` (`android_apk` with `keystore` in `buck/android/defs.bzl`) signs CI's APKs on a
worker with `buck/android/ci-keystore.base64`, a base64 keystore that git
ignores. ci.yml writes it from the repository secret
`ANDROID_CI_KEYSTORE_BASE64` before it builds. Every CI build is signed with
the same keystore so that each one installs as an update over the last; a new
keystore means every device must uninstall the CI app and launcher once.
To make a new one, signed the way `_sign` expects (store password
`android`), and copy it for the secret:

```sh
keytool -genkeypair -keystore ci.keystore -storepass android -alias androiddebugkey \
    -keypass android -dname 'CN=Android Debug,O=Android,C=US' -keyalg RSA -keysize 2048 -validity 10000
base64 -w0 ci.keystore
```

The keystore is an input of the signing action, so it is in the build server's CAS,
where anyone with its key can read it.

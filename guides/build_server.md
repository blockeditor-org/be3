# The build server

Every buck2 action runs on a remote execution server. `BE3_BUILD_SERVER` picks
which, or else the one saved in `~/.config/be3/build-server`. Without either,
at a terminal, `./scripts/buck` asks which and saves the answer there (delete
it to be asked again); anything else builds on Namespace.

| `BE3_BUILD_SERVER` | Server | Key |
|---|---|---|
| `namespace` (the default, and CI's) | Namespace's remote execution | Namespace's token (below) |
| `blocks.pfg.pw` | our own NativeLink server | `authorization: Bearer KEY` |
| `buildserver.pfg.pw` | our own NativeLink server, 8 cores | `authorization: Bearer KEY` |
| `buildbuddy` | BuildBuddy's `remote.buildbuddy.io` | `x-buildbuddy-api-key: KEY` |
| `local` | a NativeLink server of your own, at `host:port` | none, and no TLS |

`local`'s address, such as `127.0.0.1:50052`, is `BE3_BUILD_SERVER_ADDRESS`,
or else the second line of `~/.config/be3/build-server`, where the prompt
saves it. It is dialed directly, never through `scripts/internal/re-relay`.

The servers share nothing: each has its own cache and its own key.
`./scripts/buck` writes the one it picked into `.buckconfig.local`, with its
name as `be3.build_server`; `worker_properties` (`buck/tools/defs.bzl`) reads
that to pick the workers' image, since Namespace runs only its own copy of it.
`nsc` is only downloaded for Namespace.

A server other than Namespace and `local` takes its key from:

1. `BE3_BUILD_SERVER_KEY` in the environment
2. `.build-server-key.SERVER` at the root of the checkout
3. `~/.config/be3/build-server-key.SERVER`

where `SERVER` is the value of `BE3_BUILD_SERVER`. Without one, at a terminal,
`./scripts/buck` asks for it and saves it to the last; Namespace's token is
asked for and saved the same way, to `~/.config/be3/namespace-token.json`.

In an agent's cloud session the proxy adds the key to requests for
`blocks.pfg.pw` and `buildserver.pfg.pw`, and `BE3_BUILD_SERVER_KEY` is a
placeholder; there is no key for BuildBuddy there.

## When a build hangs or keeps failing

The servers fail in different ways:

- **Namespace** is the fastest, but now and then never finishes a build: an
  action sits at `[re_execute]` or queued for hours while nothing else moves.
  It does not recover on its own, so stop the command rather than wait.
- **`buildserver.pfg.pw` and `blocks.pfg.pw`** always finish, but queue
  actions when many builds share them, and are slower than Namespace then.

To move off a server that hangs or keeps failing, run the command again with
another one, for that command or, exported, for the rest of the session:

```
BE3_BUILD_SERVER=buildserver.pfg.pw ./scripts/verify
```

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
  (`credentials missing`) or refused the one it has (`invalid token`). For
  Namespace, `BE3_NAMESPACE_TOKEN` or the token file is wrong or revoked; for
  the others, the session has no key for that server. Switch to a server the
  session has a key for.

Each checkout has relays of its own (guides/buck2.md), so two checkouts on one
machine can build on different servers at once.

## Namespace

Namespace's remote execution
(https://namespace.so/docs/bazel/execution) runs on a cluster that belongs to
the repository's Namespace workspace: an executor (`reapih-…`) and the storage
that holds the action cache and the CAS (`storageh-…`).

### Waking it

Namespace shuts the cluster down when it has been idle for a while, and its
hosts then answer 404. So no host is checked in: `./scripts/buck` runs the
pinned `nsc` (`scripts/internal/install-nsc.sh`), whose
`nsc reapi setup buck2 --token FILE --key buildserver` starts the cluster if it
is down and names its hosts and the `x-nsc-ingress-auth` header that
authenticates to them. `./scripts/buck` keeps that answer in
`target/namespace/`, writes it into `.buckconfig.local`, and asks again when
the executor stops answering, when the answer is three hours old, or when the
token changes.

Setup must always be run with those arguments and that `--key`: without a
key, or with other flags (such as `--static`), Namespace starts a new cluster
beside the old one. The dashboard shows both, and the stale one should be
destroyed, since its workers count against the workspace's 32 vCPU limit.

### The token

`nsc` authenticates with a token from:

1. `BE3_NAMESPACE_TOKEN` in the environment
2. `.namespace-token.json` at the root of the checkout
3. `~/.config/be3/namespace-token.json`

Each holds what this writes, or the bare token in it, made with a Namespace
login:

```
nsc reapi create-token --no_expiry --token ns-token.json
```

A token is revoked at https://cloud.namespace.so/user/sessions.

- **CI:** `BE3_NAMESPACE_TOKEN` is the repository secret of that name.
- **An agent's cloud session:** `BE3_NAMESPACE_TOKEN` is set in the
  environment's variables. When `HTTPS_PROXY` is set, `nsc` reaches
  `private-api.global.namespaceapis.com` through it, and buck2 reaches the
  cluster through `scripts/internal/re-relay` (guides/buck2.md).

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

`:android-dist` (`signed_apk` in `buck/android/defs.bzl`) signs CI's APKs on a
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

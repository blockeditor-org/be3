# The build server

Every buck2 action runs on a remote execution server. This guide covers choosing one, its key,
what to do when a build hangs or keeps failing, Namespace's cluster and workers, and CI's
Android keystore. Read "When a build hangs or keeps failing" first if a build is stuck.

## Choosing a server

`BE3_BUILD_SERVER` picks the server, or else the one saved in `~/.config/be3/build-server`.
Without either, at a terminal, `./scripts/buck` asks and saves the answer there (delete the
file to be asked again); anything else builds on Namespace.

| `BE3_BUILD_SERVER` | Server | Key | In an agent's cloud session |
|---|---|---|---|
| `namespace` (the default, and CI's) | Namespace's remote execution | Namespace's token | works with no configuration |
| `blocks.pfg.pw` | our own NativeLink server | `authorization: Bearer KEY` | works with no configuration |
| `buildbuddy` | BuildBuddy's `remote.buildbuddy.io` | `x-buildbuddy-api-key: KEY` | currently not working |
| `local` | a NativeLink server of your own, at `host:port` | none, and no TLS | not powerful enough to host one |

- `local`'s address, such as `127.0.0.1:50052`, is `BE3_BUILD_SERVER_ADDRESS`, or else the
  second line of `~/.config/be3/build-server`, where the prompt saves it. It is dialed
  directly, never through `scripts/internal/re-relay`.
- The servers share nothing: each has its own cache and its own key.
- `./scripts/buck` writes the chosen server into `.buckconfig.local`, with its name as
  `be3.build_server`. `worker_properties` (`buck/tools/defs.bzl`) reads that to pick the
  workers' image, since Namespace runs only its own copy of it.
- `nsc` is only downloaded for Namespace.

## Keys

A server other than Namespace and `local` takes its key from the first of:

1. `BE3_BUILD_SERVER_KEY` in the environment
2. `.build-server-key.SERVER` at the root of the checkout
3. `~/.config/be3/build-server-key.SERVER`

where `SERVER` is the value of `BE3_BUILD_SERVER`. Without one, at a terminal,
`./scripts/buck` asks for it and saves it to the last. Namespace's token is asked for and
saved the same way, to `~/.config/be3/namespace-token.json` (see Namespace, The token).

In an agent's cloud session the proxy adds the key to requests for `blocks.pfg.pw`, and
`BE3_BUILD_SERVER_KEY` is a placeholder. There is no key for BuildBuddy there.

A server answers a call with a wrong or missing key or token with `UNAUTHENTICATED`, which
buck2 reports as failing to reach it.

## When a build hangs or keeps failing

The servers fail in different ways:

- **Namespace** is the fastest, but now and then never finishes a build: an action sits at
  `[re_execute]` or queued for hours while nothing else moves. It does not recover on its own,
  so stop the command rather than wait.
- **`blocks.pfg.pw`** always finishes, but runs slower.

To move off a server that hangs or keeps failing, run the command again with another one, for
that command or, exported, for the rest of the session:

```
BE3_BUILD_SERVER=blocks.pfg.pw ./scripts/verify
```

You may also need to kill buck's daemon: `./scripts/buck kill`.

- `./scripts/buck` rewrites `.buckconfig.local` and restarts the buck2 daemon on every change
  of server.
- The first build on a server rebuilds everything it has not built before, which is most of
  the workspace for a server that has not built this branch. Switching pays off for a build
  that is stuck, not for one that is merely slow.
- Each checkout has relays of its own, on ports picked from its path, so two checkouts on one
  machine can build on different servers at once.

### Errors behind an HTTPS proxy

Behind an HTTPS proxy, as in an agent's cloud session, buck2 reaches the server through
`scripts/internal/re-relay` (`guides/buck2.md`, What `./scripts/buck` does). Its errors are in
`target/re-relay.log`:

| Error | Meaning | What to do |
|---|---|---|
| `unexpected EOF`, `context canceled`, a connection reset | a call dropped on the way; buck2 fails the command with an infrastructure error (exit status 2), and `./scripts/buck` tries it three times in all (`BE3_BUCK_ATTEMPTS`), each time finding what finished cached | if it fails every time, the server is down or overloaded: switch |
| `the proxy answered 401 Unauthorized` | the proxy has no key for the host (`credentials missing`) or refused the one it has (`invalid token`); for Namespace, `BE3_NAMESPACE_TOKEN` or the token file is wrong or revoked; for the others, the session has no key for that server | switch to a server the session has a key for |

## Namespace

Namespace's remote execution (https://namespace.so/docs/bazel/execution) runs on a cluster that
belongs to the repository's Namespace workspace: an executor (`reapih-…`) and the storage that
holds the action cache and the CAS (`storageh-…`).

### Waking the cluster

Namespace shuts the cluster down when it has been idle for a while, and its hosts then answer
404, so no host is checked in.

- `./scripts/buck` installs the pinned `nsc` (`scripts/internal/install-nsc.sh`) into
  `target/tools/nsc-<version>/` and runs
  `nsc reapi setup buck2 --token FILE --key buildserver`, which starts the cluster if it is
  down and names its hosts and the `x-nsc-ingress-auth` header that authenticates to them.
- It keeps that answer in `target/namespace/`, writes it into `.buckconfig.local`, and asks
  again when the executor stops answering, when the answer is three hours old, or when the
  token changes.
- Always run setup with exactly those arguments and that `--key`. Without a key, or with other
  flags (such as `--static`), Namespace starts a new cluster beside the old one. The dashboard
  then shows both; destroy the stale one, since its workers count against the workspace's
  32 vCPU limit.

### The token

`nsc` authenticates with a token from the first of:

1. `BE3_NAMESPACE_TOKEN` in the environment
2. `.namespace-token.json` at the root of the checkout
3. `~/.config/be3/namespace-token.json`

Each holds what this writes, or the bare token in it, made with a Namespace login:

```
nsc reapi create-token --no_expiry --token ns-token.json
```

- A token is revoked at https://cloud.namespace.so/user/sessions.
- **CI:** `BE3_NAMESPACE_TOKEN` is the repository secret of that name.
- **An agent's cloud session:** `BE3_NAMESPACE_TOKEN` is set in the environment's variables.
  When `HTTPS_PROXY` is set, `nsc` reaches `private-api.global.namespaceapis.com` through it,
  and buck2 reaches the cluster through `scripts/internal/re-relay`.

### The workers' image

Actions run in the image named by `container-image` in `worker_properties`
(`buck/tools/defs.bzl`), so an action sees the same system wherever it runs.

- Namespace runs only images in its own registry, `nscr.io`, pinned by digest, that it has
  optimized. Any other is refused with `the image is not optimized for remote execution`.
- Namespace's default image cannot run the toolchain.

A new image is copied in and optimized with a Namespace login:

```
nsc base-image upload docker.io/library/IMAGE@sha256:DIGEST be3-worker:TAG
nsc base-image optimize --image_ref nscr.io/WORKSPACE/be3-worker@sha256:DIGEST
```

`upload` prints the `nscr.io` reference with its digest, which is what `optimize` and
`worker_properties` take. A new digest needs optimizing once.

### How actions run

- **As root, in the image**, in a fresh directory, without per-action sandboxing. An action
  also sees the worker's own environment, among it `NSC_TOKEN_FILE`.
- **`LD_LIBRARY_PATH` is the worker's.** The worker replaces the one an action asks for. A test
  that needs its own (the ones that draw through lavapipe) carries it as `BE3_LD_LIBRARY_PATH`,
  and the shell that starts the test puts it back: `library_path_test` in
  `buck/cargo/defs.bzl`, which `cargo_test` makes for a test whose `env` names
  `LD_LIBRARY_PATH`, and `plugin_test_run` in `buck/wasm/defs.bzl`.

## CI's Android keystore

`:android-dist` (`android_apk` with `ci_signed` in `buck/android/defs.bzl`) signs CI's APKs on
a worker with `buck/android/ci-keystore.base64`, a base64 keystore that git ignores. `ci.yml`
writes it from the repository secret `ANDROID_CI_KEYSTORE_BASE64` before it builds.

- Every CI build is signed with the same keystore so that each one installs as an update over
  the last. A new keystore means every device must uninstall the CI app and launcher once.
- The keystore is an input of the signing action, so it is in the build server's CAS, where
  anyone with the server's key can read it.

To make a new one, signed the way `_sign` expects (store password `android`), and copy it for
the secret:

```sh
keytool -genkeypair -keystore ci.keystore -storepass android -alias androiddebugkey \
    -keypass android -dname 'CN=Android Debug,O=Android,C=US' -keyalg RSA -keysize 2048 -validity 10000
base64 -w0 ci.keystore
```

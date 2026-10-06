# The build server

Every buck2 action runs on Namespace's remote execution
(https://namespace.so/docs/bazel/execution), on a cluster that belongs to the
repository's Namespace workspace. `.buckconfig` names its two hosts: the
executor (`reapih-…`) and the storage that holds the action cache and the CAS
(`storageh-…`). Only callers with the token can use them.

## The token

Each call carries `x-nsc-ingress-auth: Bearer TOKEN`; `.buckconfig` sends
`$BE3_BUILD_SERVER_KEY` as the token. `./scripts/buck` looks for it in this
order:

1. `BE3_BUILD_SERVER_KEY` in the environment
2. `.build-server-key` at the root of the checkout
3. `~/.config/be3/build-server-key`

With none of them, it asks for the token at a terminal and saves it to the
last.

Making one takes a Namespace login (`nsc login`):

```
nsc reapi create-token --no_expiry --token ns-token.json
nsc reapi setup buck2 --token ns-token.json --config ns.buckconfig
```

The token is the bearer in `ns.buckconfig`'s `http_headers`. `ns.buckconfig` also
names the hosts, if the cluster ever moves. A token is revoked at
https://cloud.namespace.so/user/sessions.

- **CI:** it reads the token from the repository secret `BE3_NAMESPACE_TOKEN`.
- **A call without the token:** Namespace refuses it with `UNAUTHENTICATED`.
- **A proxy can supply it.** It adds `x-nsc-ingress-auth: Bearer TOKEN` to
  requests for `*.iad4.namespaced.app`, and `BE3_BUILD_SERVER_KEY` can be any
  placeholder, such as `proxy-injected`. When `HTTPS_PROXY` is set, buck2
  reaches the hosts through `scripts/internal/re-relay` (guides/buck2.md).

## The workers' image

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

## How actions run

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

The keystore is an input of the signing action, so it is in Namespace's CAS,
where anyone with the token can read it.

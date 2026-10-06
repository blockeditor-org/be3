# The build server

Every buck2 action runs on Namespace's remote execution
(https://namespace.so/docs/bazel/execution), on a cluster that belongs to the
repository's Namespace workspace: an executor (`reapih-…`) and the storage that
holds the action cache and the CAS (`storageh-…`).

## Waking it

Namespace shuts the cluster down when it has been idle for a while, and its
hosts then answer 404. So no host is checked in: `./scripts/buck` runs the
pinned `nsc` (`scripts/internal/install-nsc.sh`), whose
`nsc reapi setup buck2` starts the cluster if it is down and names its hosts and
the `x-nsc-ingress-auth` header that authenticates to them. `./scripts/buck`
keeps that answer in `target/namespace/`, writes it into `.buckconfig.local`,
and asks again when the executor stops answering, when the answer is three
hours old, or when the token changes. Running setup again for a cluster that
is half shut down may start a second one; the dashboard shows both, and the
stale one should be destroyed, since the workers count against the
workspace's 32 vCPU limit.

## The token

`nsc` authenticates with a token from:

1. `BE3_NAMESPACE_TOKEN` in the environment
2. `.namespace-token.json` at the root of the checkout
3. `~/.config/be3/namespace-token.json`

With none of them it uses the person's own `nsc login`
(`target/tools/nsc-<version>/nsc login`). Each holds what this writes, or the
bare token in it, made with a Namespace login:

```
nsc reapi create-token --no_expiry --token ns-token.json
```

A token is revoked at https://cloud.namespace.so/user/sessions.

- **CI:** `BE3_NAMESPACE_TOKEN` is the repository secret of that name.
- **An agent's cloud session:** `BE3_NAMESPACE_TOKEN` is set in the
  environment's variables. When `HTTPS_PROXY` is set, `nsc` reaches
  `private-api.global.namespaceapis.com` through it, and buck2 reaches the
  cluster through `scripts/internal/re-relay` (guides/buck2.md).

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
`ANDROID_DEBUG_KEYSTORE_BASE64` before it builds. Every CI build is signed with
the same keystore so that each one installs as an update over the last; a new
keystore means every device must uninstall the CI app and launcher once.

The keystore is an input of the signing action, so it is in Namespace's CAS,
where anyone with the token can read it.

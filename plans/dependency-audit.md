# Dependency and supply-chain audit

Audit date: 2026-09-20. Scope: everything the local dev scripts and CI fetch
over the network, plus the actions CI runs. No code changed — this is the list
of things to fix.

Two questions were asked of every fetch:

1. Is the thing we download **pinned** (an exact version/commit, not a floating
   tag or `latest`)?
2. Is what actually arrives **verified** against a hash we recorded, so a
   compromised host, a moved tag, or a MITM on a proxied network can't swap it?

Most of the repository already passes (1). The gap is (2): of the six things we
download and unpack, only one checks a hash.

---

## Summary of findings

| # | Item | Pinned? | Verified? | Severity |
|---|---|---|---|---|
| 1 | `dtolnay/rust-toolchain@master` | **no** | no | **high** |
| 2 | Third-party actions on floating tags (`taiki-e`, `Swatinem`, `mlugg`, `gradle`) | tag only | no | **high** |
| 3 | Zig toolchain tarball (`scripts/internal/build-ghostty-vt.sh`) | yes | **no** | **high** |
| 4 | sccache release tarball (`scripts/internal/install-sccache.sh`) | yes | **no** | **high** |
| 5 | WASI sysroot tarball (`scripts/internal/common.sh`) | yes | **no** | **high** |
| 6 | rustup bootstrap (`curl https://sh.rustup.rs \| sh`) | **no** | no | medium |
| 7 | Gradle distribution (no wrapper, no `distributionSha256Sum`) | yes | **no** | medium |
| 8 | `cargo install wasm-bindgen-cli` without `--locked` | partly | n/a | low |
| 9 | Ghostty `zig fetch` prefetch path | commit-pinned | indirectly | low |
| 10 | Shared sccache object store as a build input | n/a | **no** | medium (design) |
| 11 | Android SDK packages via `sdkmanager --channel=3` | yes | upstream | low |
| 12 | No `dependabot.yml`, no workflow linter | — | — | low |

## What is already correct

Worth recording so a fix doesn't regress it:

- **Cargo is fully hash-verified.** `Cargo.lock` has 831 `checksum` entries, every
  package resolves to `registry+https://github.com/rust-lang/crates.io-index`,
  there are no `git =` dependencies anywhere in the workspace, and no package
  with a `source` lacks a checksum. `.cargo/config.toml` does no registry
  replacement.
- **PDFium is the model to copy.** `scripts/internal/fetch-pdfium.sh` pins the
  release (`chromium/8044`), records a per-target SHA-256, aborts and deletes the
  archive on mismatch, and its comment notes the hashes were cross-checked
  against pdfium-binaries' own build provenance attestation rather than merely
  recomputed from the same download. Every other fetch in this repo should look
  like this one.
- **Ghostty is pinned by full commit SHA** (`ghostty_commit` in
  `scripts/internal/build-ghostty-vt.sh:11`) and fetched with `git fetch --depth 1
  origin <sha>`, so git's own content addressing validates the tree. The local
  patch is keyed into the stamp via `git hash-object`.
- **The Rust toolchain is pinned** (`rust-toolchain.toml`, `channel = "1.98.0"`),
  and both composite actions derive their version from the single source of
  truth rather than repeating it.
- **Workflow permissions are tight and there is no script-injection surface.**
  `permissions: contents: read` at workflow level, no `pull_request_target`, and
  the only `github.*` expression interpolated anywhere is `github.event_name`
  / `github.ref`, never attacker-controlled text such as `github.event.*.title`
  or `head_ref`.
- **No CDN dependencies in the web bundle.** `scripts/internal/web/index.html`
  uses an import map over local modules; nothing is pulled from unpkg, jsdelivr,
  esm.sh or Google Fonts.
- **apt and rustup component installs are signature-verified by their own tooling.**

---

## 1. `dtolnay/rust-toolchain@master` — a mutable branch

`.github/actions/rust-toolchain/action.yml:36`

```yaml
- uses: dtolnay/rust-toolchain@master
```

This is the worst pin in the repo: not a tag, a branch. Every CI job on every
platform runs whatever is on that branch at the moment the job starts, and it is
the step that installs the compiler the rest of the job trusts. A compromised
upstream account, or a single bad commit, lands in our builds within minutes and
with no version bump to notice.

**Fix:** pin to a commit SHA (see the table in §2). The action is used for its
`toolchain`/`targets`/`components` inputs only, all of which have been stable
across releases, so pinning costs nothing.

## 2. Third-party actions on floating tags

A tag is a mutable pointer. `v2` can be repointed at any commit by the owner or
by anyone who compromises the account, and a run that already passed CI yesterday
picks it up silently today. GitHub's own advice is to pin third-party actions by
full commit SHA; actions under the `actions/` org are covered by GitHub's own
release process and may keep tags, which is the policy this report assumes.

SHAs resolved 2026-09-20 with `git ls-remote`; re-resolve at the time the fix is
written, and record the version each SHA corresponds to in a trailing comment so
the pin stays legible and bumpable.

| Used at | Current | Pin to | = version |
|---|---|---|---|
| `.github/actions/rust-toolchain/action.yml:36` | `dtolnay/rust-toolchain@master` | `02cb101ec7c40f2c49e1d9714d64511d8e1b74de` | v1 |
| `.github/actions/sccache/action.yml:30`, `ci.yml:123`, `ci.yml:180`, `ci.yml:233` | `taiki-e/install-action@v2` | `94c31af3204a9f15ab40b35ad084410b905bbc73` | v2.87.17 |
| `.github/actions/cargo-cache/action.yml:31` | `Swatinem/rust-cache@v2` | `6323deb102c322ba6fcbdcafc7e3dddab59af2b6` | v2.9.2 |
| `ci.yml:86,134,197,250,356,493` | `mlugg/setup-zig@v2` | `d1434d08867e3ee9daa34448df10607b98908d29` | v2.2.1 |
| `ci.yml:360` | `gradle/actions/setup-gradle@v4` | `ed408507eac070d1f99cc633dbcf757c94c7933a` | v4.4.3 |

Note `gradle/actions/setup-gradle` is a subdirectory action; `gradle/actions/setup-gradle@<sha>`
is the correct form. Note also that the floating `v4` tag currently trails the
newest release (`v4.4.4` exists), which is itself a small illustration of why a
tag is not a version.

GitHub-owned, keep on tags per the agreed policy: `actions/checkout@v7`
(6 uses), `actions/cache@v6` (4), `actions/setup-java@v5` (1),
`actions/upload-artifact@v7` (4), `actions/upload-pages-artifact@v3` (1).

**What the third-party actions do for us**, so the pinning decision is informed:
`taiki-e/install-action` and `mlugg/setup-zig` both verify what *they* download
(install-action against its own checksum manifest, setup-zig against Zig's
minisign signature), so pinning the action SHA is what closes the remaining gap
rather than duplicating their work. `Swatinem/rust-cache` and
`gradle/actions/setup-gradle` restore artifacts into the build tree, which makes
them as trusted as the compiler.

## 3. Zig toolchain tarball — downloaded with no hash

`scripts/internal/build-ghostty-vt.sh:80-89`

```bash
local url="https://ziglang.org/download/$zig_version/$name.tar.xz"
curl --fail --location --output "$archive" "$url"
tar -xJf "$archive" -C "$tools"
```

The version is pinned (`zig_version='0.16.0'`), the bytes are not checked. The
extracted `zig` then compiles libghostty-vt, which is linked into `block-app` on
every platform — so this is arbitrary-code-execution-at-build-time if the
download is tampered with.

Mitigating: in CI, `mlugg/setup-zig` puts a signature-verified Zig 0.16.0 on
PATH before this script runs, and the script's `command -v zig` check finds it
and skips the download entirely. So this path is **local developer machines and
`./scripts/setup`**, not CI. That makes it lower-blast-radius, not safe.

**Fix:** record a per-platform SHA-256 the way `fetch-pdfium.sh` does. The
authoritative values come from `https://ziglang.org/download/index.json`, which
publishes a `shasum` per tarball (fetched 2026-09-20):

| Platform | SHA-256 |
|---|---|
| `x86_64-linux` | `70e49664a74374b48b51e6f3fdfbf437f6395d42509050588bd49abe52ba3d00` |
| `aarch64-linux` | `ea4b09bfb22ec6f6c6ceac57ab63efb6b46e17ab08d21f69f3a48b38e1534f17` |
| `x86_64-macos` | `0387557ed1877bc6a2e1802c8391953baddba76081876301c522f52977b52ba7` |
| `aarch64-macos` | `b23d70deaa879b5c2d486ed3316f7eaa53e84acf6fc9cc747de152450d401489` |

Zig also publishes minisign signatures; verifying the hash against the JSON index
is the cheap 90%, and is what makes the pin meaningful.

## 4. sccache release tarball — downloaded with no hash

`scripts/internal/install-sccache.sh:44-51`

```bash
url="https://github.com/mozilla/sccache/releases/download/v$sccache_version/$release.tar.gz"
curl --fail --location --output "$archive" "$url"
tar -xzf "$archive" -C "$tools"
```

Version pinned via `sccache_version='0.18.0'` in `common.sh:540`; no hash. This
binary becomes `RUSTC_WRAPPER`, i.e. it wraps every single `rustc` invocation
and (on non-Windows) `cc`/`c++` as well. A swapped sccache sees and can alter
every compilation in the workspace. This runs on developer machines via
`./scripts/setup`; CI installs sccache through `taiki-e/install-action` instead
(`.github/actions/sccache/action.yml`), which does check a checksum.

Only two architectures are ever fetched, so two hashes cover it. Computed
2026-09-20 from the pinned URLs:

| Asset | SHA-256 |
|---|---|
| `sccache-v0.18.0-x86_64-unknown-linux-musl.tar.gz` | `45f1447fbe231e3037bde351ef70677dd212216c8d62ae7ca409fecc4d6acc89` |
| `sccache-v0.18.0-aarch64-unknown-linux-musl.tar.gz` | `2b3284d5da3b46a47dc4229e75bb7b88ac4aa99c8d754fb7d2f84997e5a4354a` |

These are recorded from a download, which is trust-on-first-use. Before landing
them, cross-check against mozilla/sccache's GitHub build provenance attestation
(`gh attestation verify`) the way the PDFium hashes were, and say so in a comment
next to them — otherwise the hash only proves the file didn't change, not that it
was ever the right file.

## 5. WASI sysroot tarball — downloaded with no hash

`scripts/internal/common.sh:383-389`

```bash
local url="https://github.com/WebAssembly/wasi-sdk/releases/download/wasi-sdk-$wasi_sdk_version/wasi-sysroot-$wasi_sdk_version.0+m.tar.gz"
curl --fail --location --output "$archive" "$url"
tar -xzf "$archive" -C "$tools"
```

Pinned (`wasi_sdk_version='33'`), unverified. This sysroot is what the web bundle
and every plugin's C dependencies are compiled and linked against — it supplies
libc, libc++ and `libsetjmp.a` for `wasm32-wasip1-threads`. Unlike items 3 and 4,
**this one runs in CI**: the web, plugins and plugin-tests jobs all fetch it on a
cache miss (`actions/cache@v6` with key `wasi-sysroot-33` only skips the
download on a hit), so it is in the trust path of the artifacts we publish.
Highest-priority of the three unhashed tarballs for that reason.

Computed 2026-09-20 from the pinned URL:

| Asset | SHA-256 |
|---|---|
| `wasi-sysroot-33.0+m.tar.gz` | `063bc1b56582b9923e08ac9b89e58789618d851763f01530b3ff20b9e5df0ca3` |

Same TOFU caveat as §4 — cross-check against upstream provenance before landing.

Note also that `wasi_sysroot_is_complete()` checks only that directories and
`libsetjmp.a` exist, so a partially-tampered or partially-extracted cached
sysroot passes. A hash check on the archive at download time plus a recorded
stamp file (the pattern `build-ghostty-vt.sh` already uses) would cover that too.

## 6. rustup bootstrap is `curl | sh`

`scripts/setup:138-141`

```bash
curl --fail --location --proto '=https' --tlsv1.2 https://sh.rustup.rs \
    | sh -s -- -y --no-modify-path
```

Unpinned and unverified: whatever `sh.rustup.rs` serves today is executed
directly, never touching disk, so it cannot even be inspected after the fact.
The `--proto '=https' --tlsv1.2` flags are good hygiene but only constrain the
transport.

**Fix (in order of preference):**

1. Pin a rustup version and fetch `rustup-init` directly from
   `https://static.rust-lang.org/rustup/archive/<version>/<host-triple>/rustup-init`,
   verify a recorded SHA-256, then execute it.
2. At minimum, download the shell script to a file, hash-check it, then run it —
   this keeps the "any recent rustup" behaviour but makes the bump explicit.

Lower severity than 3-5 only because it runs once, on a fresh machine, and never
in CI (CI gets its toolchain from `dtolnay/rust-toolchain`, which is finding #1).

## 7. Gradle distribution — no wrapper, no checksum

`ci.yml:360`

```yaml
- uses: gradle/actions/setup-gradle@v4
  with:
    gradle-version: '8.11.1'
```

There is no Gradle wrapper in the repo at all — `android/` has
`settings.gradle`, `build.gradle`, `gradle.properties` and no `gradlew` or
`gradle/wrapper/gradle-wrapper.properties`. So the version is pinned in two
unrelated places (this workflow, and the `PATH=...gradle-8.11.1/bin` incantation
in `AGENTS.md`), and nothing anywhere records a hash of the distribution. CI's
verification is whatever `setup-gradle` does internally; the local path has none.

**Fix:** add a Gradle wrapper and pin `distributionSha256Sum` in
`gradle-wrapper.properties`, then have both CI and `scripts/internal/build-android.sh`
invoke `./gradlew` rather than a `gradle` found on PATH. The published checksum
for the version already in use (from `services.gradle.org`, fetched 2026-09-20):

```
gradle-8.11.1-bin.zip  f397b287023acdba1e9f6fc5ea72d22dd63669d59ed4a289a29b1a76eee151c6
```

This also removes a real papercut: the version stops being duplicated in prose.

Related, lower risk: `android/settings.gradle` resolves plugins and dependencies
from `google()`, `mavenCentral()` and `gradlePluginPortal()` with
`com.android.application` pinned to `8.9.1`, and no Gradle dependency
verification metadata (`gradle/verification-metadata.xml`). Adding one is the
Gradle-native equivalent of `Cargo.lock`'s checksums. Worth doing after the
wrapper, not before.

## 8. `cargo install wasm-bindgen-cli` without `--locked`

`scripts/internal/build-web.sh:48-49`

```bash
cargo install wasm-bindgen-cli --version "$wasm_bindgen_version"
```

The crate version is pinned and every crate it pulls is checksum-verified by
cargo, so this is not an integrity hole — it is a reproducibility one. Without
`--locked`, the dependency graph is re-resolved to whatever satisfies semver
today, so two machines can build different binaries from the same command, and a
malicious minor release of any transitive dependency is picked up without a
version bump here. `scripts/setup:161` already gets this right
(`cargo install cargo-nextest --locked`).

**Fix:** add `--locked`. One-word change.

## 9. Ghostty dependency prefetch — verified, but only indirectly

`scripts/internal/build-ghostty-vt.sh:207-274`

When the build runs behind a proxy, `prefetch_dependencies` scrapes every
`.url` out of Ghostty's `.zon` manifests and fetches each with `curl` or `git`
(`prefetch_url` / `prefetch_git`), then hands the result to `zig fetch`. The
`curl` has no hash check of its own, and `prefetch_git`'s fallback path clones
and, when the manifest names no commit, leaves HEAD wherever the default branch
points today.

This is **probably fine**, because Zig's package cache is content-addressed and
`build.zig.zon` pins each dependency by hash: a tampered tarball hashes
differently, lands under a different cache key, and the build then fails to find
the pinned package rather than silently using the bad one. That makes it a
build failure, not a compromise.

**Action:** confirm that reasoning against the Ghostty commit we pin (check that
every `.dependencies` entry in its `.zon` files carries a `.hash`), and if it
holds, write a one-line note saying so where the prefetch is defined, so the next
reader doesn't have to re-derive it. If any dependency turns out to be
hash-less, it needs a pin of its own. No code change expected.

## 10. The shared sccache store is an unverified build input

`scripts/internal/common.sh:540-615`

Every build — local and CI — routes `rustc` (and `cc`/`c++` off Windows) through
sccache against a Bunny S3 bucket, reading with a password committed in the repo
(`sccache_read_only_password`, line 543) and writing when `BUNNY_SCCACHE_PASSWORD`
is present. Cached object files are consumed as compiler output with no
signature or hash beyond sccache's own cache key.

This is a deliberate design trade and not a "download without a hash" in the
sense of the rest of this report, but it belongs in the same threat model: anyone
holding the write password, or anyone who can write to that bucket, can serve a
poisoned object for any crate and it ends up linked into a release artifact. The
read-only password being public is fine on its own; the point is that the
write side is a single shared secret with no second check.

**Not proposing a change here** — it would cost most of what the cache buys.
Recording it so the trade is explicit, plus two cheap hardening options if we
ever want them: (a) don't use the shared store for the jobs that produce
uploaded artifacts, only for lint/test; (b) rotate `BUNNY_SCCACHE_PASSWORD` on a
schedule. Worth confirming the read-only password really is read-only on the
Bunny zone, since it is public.

## 11. Android SDK packages

`ci.yml:328-336` installs `platform-tools`, `platforms;android-35`,
`build-tools;35.0.0` and `ndk;29.0.14206865` through `sdkmanager`, with
`yes | sdkmanager --licenses`. Versions are pinned exactly, and `sdkmanager`
verifies each package against the checksum in Google's repository manifest, so
integrity is delegated to upstream tooling over HTTPS — acceptable, and there is
no practical way to pin our own hashes here.

One nit: `--channel=3` selects the **canary** channel. The comment explains why
(the canary channel also lists stable packages), but since all four packages are
pinned to stable versions, it would be tighter to drop the flag or use the
lowest channel that resolves them, so a typo in a version string can't silently
resolve to a canary build.

## 12. Missing guardrails

- **No `.github/dependabot.yml`.** Once actions are pinned by SHA, pins go stale
  silently — which is the standard objection to SHA pinning, and Dependabot is
  the standard answer: it bumps `uses:` SHAs and keeps the `# vX.Y.Z` comment in
  sync, as a reviewable PR. Adding it should be part of the same change as §2,
  not a follow-up, or the pins will rot.
- **No workflow linter.** `zizmor` (actions-specific security lint; it flags
  exactly the unpinned-action and injection classes above) and/or `actionlint`
  would keep §2 from regressing. Either could run as a job in `ci.yml`, or from
  `./scripts/verify --lint` if we want it locally too.
- **No policy written down.** `AGENTS.md` has no line about this. Suggest adding
  one to the "Do" list: anything downloaded from a URL is pinned to an exact
  version and checked against a recorded SHA-256; third-party actions are pinned
  by commit SHA; `actions/*` may use tags.

---

## Suggested order of work

Grouped so each lands as a coherent, reviewable change.

1. **Actions pinning** (§1, §2) + `dependabot.yml` (§12). Pure config, no build
   behaviour changes, and it closes the two highest-severity findings.
2. **Hash the CI-path download** (§5, WASI sysroot). Smallest code change with
   the largest real-world effect, since it is the only unhashed tarball that
   touches published artifacts.
3. **Hash the developer-path downloads** (§3 Zig, §4 sccache). Same pattern; the
   hash-checking helper in `fetch-pdfium.sh` (`hash_of`, with the `sha256sum` /
   `shasum` fallback for macOS) should move into `common.sh` and be shared by all
   four call sites rather than copied.
4. **rustup bootstrap** (§6) and `--locked` (§8). Both small.
5. **Gradle wrapper with `distributionSha256Sum`** (§7). Largest change of the
   set because it touches the Android build path locally and in CI.
6. **Optional:** workflow linter (§12), Gradle dependency verification metadata
   (§7), the Ghostty prefetch note (§9), the `--channel=3` nit (§11).

Steps 2-4 want a shared helper, so doing them as one change is probably cheaper
than three; the report keeps them separate because their blast radii differ and
step 2 is worth landing on its own if the rest stalls.

## Verification for the fix

Every hash added should be proven to fail closed, not just to pass: corrupt the
downloaded archive (or flip a byte of the expected hash) and confirm the script
aborts, deletes the bad archive, and says which file mismatched — the way
`fetch-pdfium.sh:109-114` already does. A hash check that is never observed to
reject anything is a hash check that might be comparing two empty strings.

For the action pins, a full CI run on the branch is the check: a mistyped SHA
fails immediately and loudly, which is the one nice property of this class of
change.

# The bootstrap

Every script in `scripts/` is a few lines of `sh` that runs `crates/be-tool-bootstrap`:
`./scripts/buck`, `test`, `verify`, `ci`, `be`, `deploy`, `setup` and
`switch` are `bootstrap buck ...`, `bootstrap test ...` and so on. It is
everything that has to happen before buck2 can build: installing buck2,
picking and waking the build server (guides/build_server.md), the build id,
retries, cutting buck2's output down, finding a test's crate, and keeping
Cargo.lock and crates.bzl's `downloads` in step with the manifests (the lock
step, below). Nothing can
build it at that point, so it is a prebuilt release, and nothing is compiled
on the machine that runs it, except on the cargo route under Recovery.

Where code belongs:

- before a buck2 command, or deciding what it builds: `crates/be-tool-bootstrap`;
- after a buck2 command, reading what it built: `crates/be-tool-verify` for
  `./scripts/verify` and `./scripts/ci`, and `crates/be-tool-local` for the rest
  (`ci` for the Android builds and paint previews, `deploy`, `fuzz`), each built
  for this machine in that same command (guides/buck2.md), except for
  `./scripts/test`, which only prints the output `buck2 test` wrote into its
  event log, or under `--update` what each test's action kept and the
  paintings it accepted, so the bootstrap does that and nothing is downloaded
  to run it;
- inside an action on a worker: a tool the action depends on, built by buck2.

A command is one buck2 invocation, which the bootstrap runs;
`BE3_BOOTSTRAP=source` (below) is the one command that takes two.

Every question the bootstrap or `crates/be-tool-local` asks at a terminal goes
through `crates/be-lib-prompt`: a menu moved with
the arrow keys, a line of text checked as it is entered, and a secret, shown as
`*`. `be_lib_prompt::Saved` is a key that comes from an environment variable, a file
at the root of the checkout or `~/.config/be3`, or else is asked for and saved
there. Without a terminal nothing is asked, and the caller says which variable
to set.

## The release

`scripts/internal/pins` names everything the scripts download, as
`name='value'` lines that both the stub and the bootstrap read: buck2's
version, be's release, nsc's, and the bootstrap's: `bootstrap_sha256_<triple>`
is the SHA-256 of each platform's binary, which is also its name at
`https://be3-ci.b-cdn.net/bootstrap/<sha256>`.
`scripts/internal/bootstrap.sh` downloads the one for this machine into
`target/tools/bootstrap-<sha256>/`, checks it against the pin on every run (a
file there that does not match is downloaded again), and runs it.

`//crates/be-tool-bootstrap:dist` is the release for Linux, macOS and Windows on
x86_64 and arm64, in the release profile, laid out as
`<triple>/bootstrap(.exe)`. Building it again gives the same files (lld-link
gets `/Brepro`, buck/toolchains/BUCK), so a pin says whether the checkout's
release is the one published.

## Changing it

Edit the crate, then run it from source: `BE3_BOOTSTRAP=source
./scripts/buck ...` has the pinned release build `//crates/be-tool-bootstrap:bin` for
this machine and run that in its place, so a command costs two buck2
invocations. Only `./scripts/ci --bootstrap DIR` builds `:dist`, and compares each
binary with its pin on a worker, which downloads nothing while they match. Six
release builds, four of them for Windows and macOS, cost about 600 actions for a
crate that seldom changes, so nothing else builds them; after changing it run
`./scripts/ci --bootstrap DIR` to see it build for every platform.

CI publishes the release; nothing is published by hand. On a pull request
from this repository that changes what the release is built from (the paths
`BOOTSTRAP_INPUTS` names in .github/workflows/ci.yml), CI runs
`./scripts/ci --bootstrap DIR`, which has that comparison keep each binary
whose hash is not pinned, so only a changed release is downloaded;
`crates/be-tool-verify` writes those into `DIR` under their SHA-256 and
rewrites their pins. The `publish-bootstrap` job uploads them to the be3-ci Bunny zone, and the
autofix job, which waits for it, pushes the new pins to the pull request. A
name that is its content's hash never changes what it points at, and nothing
deletes them, so every commit's pins keep working.

Every commit is verified with the release pinned before it, so whatever that
release passes to, reads from or writes into the tree (verify.bxl's parameters and the
manifest it prints, test.bxl's parameters and its
`target<TAB>directory` lines, each directory holding `log`, `status` and
`changed/` as crates/be-tool-run-test's `--record` writes them,
be-tool-verify's flags, the names in scripts/internal/pins,
build-info.json's path and keys, env vars) has to keep working with it, or that
commit's verify fails. Renaming one takes steps, each landing after CI pins
the release built from the step before: first the bootstrap speaks the new name
while the tree accepts both, then the tree drops the old one. Or, in one step, the
pull request sets `BE3_BOOTSTRAP: cargo` in .github/workflows/ci.yml, so its CI runs the
checkout's own bootstrap, and drops it in a commit of its own once the autofix job has pinned
the release that run published. A value the
release writes that nothing in verify checks, such as build-info.json's
`program_id` or default server, changes in one step: until the new pin lands
that branch's builds carry the old value, and `./scripts/deploy`, the one check
on it, runs from main after the pin. A directory the
release writes into needs a tracked file to keep it in a checkout until a
release that no longer writes there is pinned.

## The lock step

`bootstrap lock [--check]` (crates/be-tool-bootstrap/src/lock.rs) is what
buckify's action cannot do without the network, and needs no particular host:
it brings Cargo.lock up to date with the Cargo.toml files, as `cargo update
--workspace` does, and pins each crate it names in buck/cargo/crates.bzl's
`downloads` (guides/buck2.md). `./scripts/verify`, `./scripts/ci` and
`//:buckify` run it before buck2, which reads those `downloads` from the start;
it does nothing when `buck/cargo/lock.sha256` is the hash of what it reads (the
root and `crates/*/Cargo.toml`, Cargo.lock, the `downloads`, the cargo
release), which takes no cargo, and under `--check` it fails instead of
writing. Otherwise it runs rust-toolchain.toml's cargo, downloaded once into
`target/tools/cargo-<version>/` and checked against `cargo_sha256_<triple>` and
`cargo_size_<triple>` in scripts/internal/pins, with its own `CARGO_HOME` under
`target/tools/` and the bootstrap itself as its `RUSTC`, answering only `-vV`,
which is all cargo asks when resolving. Each crate new to Cargo.lock is
downloaded from crates.io, checked against Cargo.lock's checksum and pinned with
its size, and the hash is written last. `crates/be-lib-crate-pins` reads and
writes the `downloads` for it and for be-tool-buck's generate alike. Moving
rust-toolchain.toml's channel means pinning that release's cargo too, each
platform's hash as `channel-rust-<version>.toml` lists it.

## Recovery

When the release cannot be had (the CDN is down, `scripts/internal/pins` has no
release for this machine, or curl is missing), build the crate with cargo:
`BE3_BOOTSTRAP=cargo ./scripts/buck ...` (any script) has `bootstrap.sh` run
`cargo build --locked --release -p be-tool-bootstrap --target-dir
target/tools/bootstrap-cargo` and run that binary in the release's place. It
needs rustup, which reads the toolchain from rust-toolchain.toml, and
crates.io, and takes a few seconds once built. The workspace's Cargo.toml and
Cargo.lock still build this one crate, though cargo builds nothing else here;
the manual broad check (.github/workflows/broad.yml) builds it the same way to
see that this keeps working. `--locked` refuses a Cargo.lock out of date with the manifests, so after
changing one, run the lock step with the binary built before
(`target/tools/bootstrap-cargo/release/bootstrap lock`). The binary finds the checkout from its own path,
so `target/tools/bootstrap-cargo/release/bootstrap COMMAND ...` runs it too.

A cargo build is not byte for byte the release `:dist` builds, so nothing it
makes is a pin: pins come only from CI (above).

buck2 is downloaded by the bootstrap, from GitHub's release and then the
mirror. When both fail, fetch it by hand: download
`https://lfs.pfg.pw/by-name/be3/<buck2_version>/buck2-<triple>.zst` (or the
same file from `https://github.com/facebook/buck2/releases/download/<buck2_version>/`),
check that its SHA-256 is the pin's `buck2_sha256_<triple>`, which is the hash
of the `.zst`, and decompress it with `zstd -d` to
`target/tools/buck2-<buck2_version>/buck2` (`buck2.exe` on Windows), executable.
The bootstrap uses whatever executable is there without hashing it.

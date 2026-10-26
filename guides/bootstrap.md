# The bootstrap

Every script in `scripts/` is a few lines of `sh` that runs `crates/bootstrap`:
`./scripts/buck`, `test`, `verify`, `ci`, `be`, `deploy`, `setup` and
`switch` are `bootstrap buck ...`, `bootstrap test ...` and so on. It is
everything that has to happen before buck2 can build: installing buck2,
picking and waking the build server (guides/build_server.md), the build id,
retries, cutting buck2's output down, and finding a test's crate. Nothing can
build it at that point, so it is a prebuilt release, and nothing is compiled
on the machine that runs it.

Where code belongs:

- before a buck2 command, or deciding what it builds: `crates/bootstrap`;
- after a buck2 command, reading what it built: `crates/verify` for
  `./scripts/verify` and `./scripts/ci`, and `crates/local-tools` for the rest
  (`ci` for the Android builds and paint previews, `deploy`, `fuzz`), each built
  for this machine in that same command (guides/buck2.md), except for
  `./scripts/test`, which only prints what each test's action kept and copies
  the paintings it accepted, so the bootstrap does that and nothing is
  downloaded to run it;
- inside an action on a worker: a tool the action depends on, built by buck2.

A command is one buck2 invocation, and the bootstrap runs it; the one
exception is `BE3_BOOTSTRAP=source`, below.

Every question the bootstrap or `crates/local-tools` asks at a terminal goes
through `crates/prompt`: a menu moved with
the arrow keys, a line of text checked as it is entered, and a secret, shown as
`*`. `prompt::Saved` is a key that comes from an environment variable, a file
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
`target/tools/bootstrap-<sha256>/`, checks it, and runs it.

`//crates/bootstrap:dist` is the release for Linux, macOS and Windows on
x86_64 and arm64, in the release profile, laid out as
`<triple>/bootstrap(.exe)`. Building it again gives the same files (lld-link
gets `/Brepro`, buck/toolchains/BUCK), so a pin says whether the checkout's
release is the one published.

## Changing it

Edit the crate, then run it from source: `BE3_BOOTSTRAP=source
./scripts/buck ...` has the pinned release build `//crates/bootstrap:bin` for
this machine and run that in its place, so a command costs two buck2
invocations. `./scripts/verify` builds `:dist` and compares each binary with
its pin on a worker, which downloads nothing while they match; it says when one
does not, and `./scripts/verify --check` fails then.

Nobody publishes by hand. On a pull request from this repository, CI runs
`./scripts/ci --bootstrap DIR`, which has that comparison keep each binary
whose hash is not pinned, so only a changed release is downloaded;
`crates/verify` writes those into `DIR` under their SHA-256 and rewrites their
pins. The
`publish-bootstrap` job uploads them to the be3-ci Bunny zone, and the
autofix job, which waits for it, pushes the new pins to the pull request. A
name that is its content's hash never changes what it points at, and nothing
deletes them, so every commit's pins keep working.

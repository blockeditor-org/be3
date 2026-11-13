# buck2 on this machine

When no build server is reachable, buck2 can run every action locally: `BE3_BUILD_SERVER=local`,
or `local` saved as the first line of `~/.config/be3/build-server` (guides/build_server.md).
`./scripts/buck` then writes this into `target/build-server.buckconfig`:

    [build]
    execution_platforms = root//buck/platforms:execution_platforms_local
    [buck2_re_client]
    enabled = false
    [fbcode]
    disable_re_tests = true
    [be3]
    rustc_threads = 2

For other settings, write them into `.buckconfig.local` by hand instead (git ignores it, and
nothing but a person writes it); while it exists `./scripts/buck` writes no
`target/build-server.buckconfig` and asks for no build server.

`execution_platforms_local` (buck/platforms/BUCK) is the build server's execution platform with
remote execution and the remote cache turned off. It takes the remote platform's label and
constraints, so the exec configurations, and with them the output paths, are the same as on a
build server. `fbcode.disable_re_tests` is the prelude's switch that runs tests here rather than
on the worker toolchains//:remote_test_execution names. The rules tell a local build apart
only by `execution_platforms` (`LOCAL_BUILD` in buck/platforms/profile.bzl), in any spelling of
`root//buck/platforms:execution_platforms` or `:execution_platforms_local`, and fail to load on
any other value, for the two differences below. Then use `./scripts/buck` as usual, such as
`./scripts/buck run //crates/be-host:dev` (guides/running/drive.md).

`be3.rustc_threads` is how many threads rustc's frontend runs on (`-Zthreads`). A build server
always uses eight, whatever the config says, so the cache never splits by who asked. Here buck2
already runs one rustc per core, and eight threads each oversubscribe the CPU and memory: on a
machine of 8 cores and 15 GB, shared with other builds, use 1 or 2. Left out, it is eight.

A local build also leaves out buck2's content-based output paths for rust targets (`RUST_LOCAL`
in buck/platforms/profile.bzl), and `program` outputs always do. With them, buck2 writes each
output where its action ran and copies it to a path named for its hash, so every rlib is stored
twice, and each change to a crate leaves the previous copy under its old hash: for one small
crate this measured 6% more than without them, plus the size of its rlib for every edit. The
rules that are not rust, such as the sysroots, `__precompiled__` and the downloads, still use them,
so old versions of those pile up too: in one day of agents' builds here, buck-out reached 49 GB,
about 20 GB of it in the content-based layout (`buck-out/v2/art/root/{crates,buck,third-party}`),
of which 8.5 GB was those scratch copies. The host tests share one `be-host` and one directory
of libraries (guides/testing_a_gui.md, 6).

.buckconfig has buck2 delete outputs no build used for three days (`clean_stale_*` under
`[buck2]`). To free space sooner, `./scripts/buck clean --stale 12h` deletes what no build used
in the last twelve hours (`--dry-run` says how much first), and `./scripts/buck clean` deletes
everything.

Everything downloads and builds here: the toolchains and sysroot (about 500 MB), then the crates.
A first `:dev-host-text` build (`-c be3.apps=text`) took six minutes on 8 cores.

`./scripts/verify` runs whole here, with nothing skipped: every compile, lint and test, the host
tests, and the browser tests (`web_drive_test`, `page_test` in buck/program/drive.bzl), which run
here like every action, with the pinned Node and headless Chromium (buck/tools) and Chromium's libraries from
`buck/sysroot:amd64-browser`: nothing to install. Lavapipe, the D-Bus daemon and the XKB keymaps come from
`buck/sysroot:amd64-test`.
On 8 cores with `rustc_threads = 2`, measured:

- **Nothing changed, daemon running**: 3 seconds.
- **A few build-file changes, daemon running**: about 6 minutes (1,070 actions).
- **A new daemon** (after `./scripts/buck kill` or a reboot), buck-out full: about 20 minutes.
  buck2 has no local action cache, so a new daemon runs every action again (11,500 of them);
  only downloads survive. Keep the daemon running between verifies.

The slowest actions in a full run: libghostty-vt's Zig build (2 minutes), be-host's precompiled
app libraries and cranelift-codegen (about a minute each), then the browser tests of beui-tests
and beui-demo-html and be-host-stack's tests (40 to 50 seconds).

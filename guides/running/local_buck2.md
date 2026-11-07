# buck2 on this machine

When no build server is reachable, buck2 can run every action locally. Write `.buckconfig.local`
by hand (git ignores it; a hand-written file makes `./scripts/buck` leave buck2's configuration alone,
guides/build_server.md):

    [build]
    execution_platforms = root//buck/platforms:execution_platforms_local
    [buck2_re_client]
    enabled = false
    [fbcode]
    disable_re_tests = true
    [be3]
    rustc_threads = 2

`execution_platforms_local` (buck/platforms/BUCK) is the build server's execution platform with
remote execution and the remote cache turned off. It takes the remote platform's label and
constraints, so the exec configurations, and with them the output paths, are the same as on a
build server. `fbcode.disable_re_tests` is the prelude's switch that runs tests here rather than
on the worker toolchains//:remote_test_execution names. The rules tell a local build apart only by
`execution_platforms` (`LOCAL_BUILD` in
buck/platforms/profile.bzl), for the two differences below. Then use `./scripts/buck` as usual,
such as `./scripts/buck run -c be3.apps=text //crates/be-host:dev` (guides/running/drive.md).

`be3.rustc_threads` is how many threads rustc's frontend runs on (`-Zthreads`). A build server
always uses eight, whatever this file says, so the cache never splits by who asked. Here buck2
already runs one rustc per core, and eight threads each oversubscribe the CPU and memory: on a
machine of 8 cores and 15 GB, shared with other builds, use 1 or 2. Leave it out for eight.

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
A first `:dev-host` build with one app took six minutes on 8 cores. `./scripts/verify` builds every
target and needs far more, so build and test only the targets you changed. Delete the file to go back to a
build server.

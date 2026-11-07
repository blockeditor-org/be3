# buck2 on this machine

When no build server is reachable, buck2 can run every action locally. Write `.buckconfig.local`
by hand (git ignores it; a hand-written file makes `./scripts/buck` leave buck2's configuration alone,
guides/build_server.md):

    [be3]
    build_server = none
    [buck2_re_client]
    enabled = false

`be3.build_server = none` makes buck/platforms/defs.bzl's execution platform local only. Without it
buck2 tries to upload each action. Then use `./scripts/buck` as usual, such as
`./scripts/buck run -c be3.apps=text //crates/be-host:dev` (guides/running/drive.md).

With no build server, rust targets also leave out buck2's content-based output paths
(`RUST_LOCAL` in buck/platforms/profile.bzl) and `program` outputs always do: buck2 writes a
content-based output where the action ran and copies it to a path named for its hash, which
doubled what buck-out held. Only a few rules that are not rust, such as the sysroots, still do.
The host tests share one `be-host` and one directory of libraries (guides/testing_a_gui.md, 6).

Everything downloads and builds here: the toolchains and sysroot (about 500 MB), then the crates.
A first `:dev-host` build with one app took six minutes on 8 cores. `./scripts/verify` builds every
target and needs far more, so build and test only the targets you changed. Delete the file to go back to a
build server.

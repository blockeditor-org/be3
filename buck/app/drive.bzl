# A beui program started headless for an agent to drive with `drive`
# (guides/running/drive.md): `./scripts/buck run //crates/<crate>:dev`. Arguments
# after `--` go to `be-drive launch`, which passes on the ones it does not know.
def headless(name, program, app_name, args = [], data = False):
    native.command_alias(
        name = name,
        args = [
            "$(location //crates/be-drive:be-drive-bin)",
            "launch",
            "--name=" + app_name,
            "--libraries=$(location //buck/sysroot:amd64)/usr/lib/x86_64-linux-gnu",
            "--program=" + program,
        ] + (["--data"] if data else []) + args,
    )

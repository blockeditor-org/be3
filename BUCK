load("//buck/tools:defs.bzl", "tool")

# clippy.toml reaches the rust toolchain as a file, so it needs a target.
export_file(
    name = "clippy.toml",
    src = "clippy.toml",
    visibility = ["PUBLIC"],
)

# The commands a person runs, as targets `./scripts/buck run` builds what they
# need for and then starts here, from the repository's root. Each is a script
# in buck/dev, which says what it does; what follows the script is what buck2
# builds or downloads for it first.
#
#   ./scripts/buck run //:verify [-- --check] [-- --lint|--tests|--plugin-tests]
#   ./scripts/buck run //:check
#   ./scripts/buck run //:buckify
#   ./scripts/buck run //:rust-project
tool(
    name = "verify",
    command = [
        "python3",
        "$(location //buck/dev:verify.py)",
        "--rustfmt",
        "$(location //buck/tools:rustfmt-sysroot)/bin/rustfmt",
        "--starlark-fmt",
        "$(location //buck/tools:starlark_fmt)",
        "--fix-rust-source",
        "$(location //crates/fix-rust-source:fix-rust-source-bin)",
        # verify.py imports it from beside itself.
        "--clippy-module",
        "$(location //buck/dev:clippy.py)",
    ],
)

tool(
    name = "check",
    command = [
        "python3",
        "$(location //buck/dev:check.py)",
    ],
)

tool(
    name = "buckify",
    command = [
        "python3",
        "$(location //buck/dev:buckify.py)",
    ],
)

tool(
    name = "rust-project",
    command = [
        "python3",
        "$(location //buck/dev:rust_project.py)",
        "$(location //buck/cargo:rust-project)",
        "$(location //buck/cargo:analyzer-sysroot)",
    ],
)

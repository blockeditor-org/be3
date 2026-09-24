# Writes rust-project.json, which is how rust-analyzer reads the workspace
# without cargo.
#
# buck2's rust-project walks the build graph for every crate under crates/ and
# writes the crates, their dependencies, features, cfgs and editions as buck2
# builds them. rust-analyzer prefers a rust-project.json over Cargo.toml when
# it finds one at the root, and runs `rust-project check` on save for
# diagnostics, which builds the saved file's target on BuildBuddy.
#
# The standard library comes from buck/cargo:analyzer-sysroot, a download of
# rust-toolchain.toml's toolchain with its sources, so this needs no rustup.
# Both it and rust-project are what //:rust-project's command names, built on a
# worker and fetched before this starts. Run this again after adding a crate or
# a dependency; the file is ignored by git, because the paths in it are this
# checkout's.
#
# Usage:
#   ./scripts/buck run //:rust-project

import json
import os
import subprocess
import sys

rust_project, sysroot = sys.argv[1:]
repository = os.getcwd()
buck = os.path.join(repository, "scripts", "buck")


def develop(*arguments):
    result = subprocess.run(
        [rust_project, "develop", "--sysroot", sysroot, "--buck2-command", buck, "--stdout"] + list(arguments),
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        text=True,
    )
    if result.returncode != 0:
        sys.stderr.write(result.stderr)
        sys.exit(result.returncode)
    return json.loads(result.stdout)


# Twice, because the plugins are a build of their own: every editor and
# block-editor-plugin are only ever compiled for wasm32-wasip1-threads, as the
# guest, which is a configuration the host's graph does not have. rust-analyzer
# needs to be told the target, or it evaluates their cfg(target_arch = "wasm32")
# code as the host's and treats it as disabled.
print("Reading the build graph...", flush=True)
project = develop("//crates/...")
guest = develop(
    "--mode=--target-platforms=root//buck/platforms:wasi_guest",
    "--rustc-target",
    "wasm32-wasip1-threads",
    "//crates/editors/...",
    "//crates/block-editor-plugin:",
)

offset = len(project["crates"])
for crate in guest["crates"]:
    for dependency in crate["deps"]:
        dependency["crate"] += offset
    project["crates"].append(crate)

# rust-project's runnables name a `buck` on PATH, which here is ./scripts/buck,
# and it adds no flycheck, so rust-analyzer would fall back to `cargo check` on
# save; rust-project's own check builds the saved file's target through buck2.
runnables = [runnable for runnable in project.get("runnables", []) if runnable["kind"] != "flycheck"]
for runnable in runnables:
    if runnable["program"] == "buck":
        runnable["program"] = buck
runnables.append(
    {
        "program": rust_project,
        "args": ["check", "--buck2-command", buck, "{saved_file}"],
        "cwd": repository,
        "kind": "flycheck",
    }
)
project["runnables"] = runnables
# rust-analyzer does not work the standard library's sources out from the
# sysroot for a rust-project.json, and without them every std type is unknown.
project["sysroot_src"] = project["sysroot"] + "/lib/rustlib/src/rust/library"
with open("rust-project.json", "w") as file:
    json.dump(project, file)
print("Wrote rust-project.json.")

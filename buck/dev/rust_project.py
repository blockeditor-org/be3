# Writes rust-project.json, which rust-analyzer reads the workspace through
# without cargo: buck2's rust-project walks the build graph, the standard
# library comes from buck/cargo:analyzer-sysroot, and diagnostics on save come
# from `rust-project check`. The file is this checkout's, so git ignores it.
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


# Twice, because the plugins are only ever built for wasm32-wasip1-threads, a
# configuration the host's graph does not have; told the target, rust-analyzer
# treats their cfg(target_arch = "wasm32") code as live.
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

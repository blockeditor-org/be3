# Regenerates the three files the build reads that are made from somewhere
# else, and writes them into the checkout:
#
#   - third-party/rust/BUCK, the rules for every third-party crate, from
#     reindeer;
#   - buck/cargo/crates.bzl, what each workspace crate's Cargo.toml says, from
#     cargo's own plan for each build buck2 stands in for;
#   - buck/sysroot/packages.bzl, every Ubuntu package the sysroot is made of,
#     resolved from what buck/sysroot/BUCK asks for.
#
# Cargo.toml stays the single place a dependency is declared, and
# buck/sysroot/BUCK the single place a system library is; run this after
# changing either. They are checked in, because a build should not have to run
# cargo or apt, and because the diff is what says what a change did. CI fails
# if any is stale.
#
# All three are generated on BuildBuddy's workers - reindeer is built there
# too - and each is keyed on what it reads, so a checkout where none of that
# changed gets the answers from the cache in seconds.
#
# Usage:
#   ./scripts/buck run //:buckify

import os
import re
import shutil
import subprocess
import sys

if len(sys.argv) != 1:
    sys.exit("Usage: ./scripts/buck run //:buckify")

buck = os.path.abspath("scripts/buck")


def run(arguments):
    result = subprocess.run(arguments, stdout=subprocess.PIPE, stderr=subprocess.STDOUT, text=True)
    if result.returncode != 0:
        sys.stdout.write(result.stdout)
        sys.exit(result.returncode)
    return result.stdout


print("Generating third-party/rust/BUCK and buck/cargo/crates.bzl on BuildBuddy...", flush=True)
output = run([buck, "bxl", "//buck/cargo/buckify.bxl:main"])
generated = re.findall(r"(/\S*/buckify\.bxl/\S*/generated)$", output, re.MULTILINE)[-1]
shutil.copyfile(os.path.join(generated, "BUCK"), "third-party/rust/BUCK")
shutil.copyfile(os.path.join(generated, "crates.bzl"), "buck/cargo/crates.bzl")

print("Resolving the sysroot packages on BuildBuddy...", flush=True)
output = run([buck, "build", "//buck/sysroot:lock", "--show-full-output"])
lock = [line.split()[1] for line in output.splitlines() if line.startswith("root//buck/sysroot:lock ")][-1]
shutil.copyfile(lock, "buck/sysroot/packages.bzl")
print("Done.")

# Fixes Rust source layout, strips comments, lints, formats and tests: what
# `./scripts/buck run //:verify` runs.
#
# By default source layout and comments are fixed, clippy applies the fixes it
# can, rustfmt rewrites the sources and the plugin tests accept whatever
# painting they produce. With --check nothing is written and each tool only
# reports, which is what CI wants. Either way the run fails if a Rust source
# violation or clippy warning survives.
#
# Everything is buck2's: the tools this runs - rustfmt, starlark_fmt,
# fix-rust-source - are the ones //:verify's command names, which buck2 builds
# or downloads before it starts this; clippy runs and every test binary is
# built on BuildBuddy's workers, and most tests run there too.
#
# A run that writes is a person's, and it is quiet: what they want out of it is
# the failures. --check is CI's, and it is loud, because the only thing left of
# that run is its log and the question asked of it afterwards is where the time
# went. --ci is the loud half of --check without the read-only half: every tool
# writes its fixes and the tests accept what they paint, but the output is the
# one CI keeps, so what the fixes changed can be pushed back to a pull request.
#
# The work splits into three parts that need nothing from one another: the lint
# pass, the tests buck2 runs on BuildBuddy's workers, and the plugin tests,
# which run here because they read and write the accepted paintings in
# snapshots/. Naming one or more of them runs only those, which is how CI gets
# them onto three runners at once. Naming none runs all three.
#
# Usage:
#   ./scripts/buck run //:verify
#   ./scripts/buck run //:verify -- --check
#   ./scripts/buck run //:verify -- --ci --lint
#   ./scripts/buck run //:verify -- --tests --plugin-tests

import argparse
import glob
import os
import shutil
import subprocess
import sys
import time

import importlib.util

# Starlark files a person wrote. The files //:buckify writes are left out,
# because a formatter that rewrote them would put it and the check that they
# are current permanently at odds.
STARLARK_ROOTS = ["buck", "crates", "third-party/system", "third-party/pdfium", "BUCK", "PACKAGE"]
GENERATED = {"buck/cargo/crates.bzl", "buck/sysroot/packages.bzl"}


def rust_files():
    # Every Rust file in the crates rather than each crate's root, because
    # rustfmt cannot see a module a macro declares, and block-client declares
    # its blocks' that way.
    listed = subprocess.run(
        ["git", "ls-files", "--cached", "--others", "--exclude-standard", "--", "crates/*.rs"],
        check=True,
        stdout=subprocess.PIPE,
        text=True,
    ).stdout.split()
    return [path for path in listed if os.path.isfile(path)]


def starlark_files():
    found = []
    for root in STARLARK_ROOTS:
        if os.path.isfile(root):
            found.append(root)
            continue
        for directory, _, files in os.walk(root):
            for name in files:
                if name == "BUCK" or name.endswith((".bzl", ".bxl")):
                    path = os.path.join(directory, name)
                    if path not in GENERATED:
                        found.append(path)
    return sorted(found)


class Run:
    def __init__(self):
        self.failed = False

    def step(self, name, action):
        print("{}...".format(name), flush=True)
        started = time.monotonic()
        try:
            passed = action()
        except subprocess.CalledProcessError:
            passed = False
        print("  {} took {:.1f}s".format(name, time.monotonic() - started), flush=True)
        if not passed:
            self.failed = True

    def command(self, arguments, **options):
        return subprocess.run(arguments, **options).returncode == 0


def main():
    parser = argparse.ArgumentParser(prog="./scripts/buck run //:verify --")
    parser.add_argument("--check", action="store_true", help="report, and write nothing")
    parser.add_argument("--ci", action="store_true", help="write, with CI's output")
    parser.add_argument("--lint", action="store_true")
    parser.add_argument("--tests", action="store_true")
    parser.add_argument("--plugin-tests", action="store_true")
    parser.add_argument("--rustfmt", required=True, help=argparse.SUPPRESS)
    parser.add_argument("--starlark-fmt", required=True, help=argparse.SUPPRESS)
    parser.add_argument("--fix-rust-source", required=True, help=argparse.SUPPRESS)
    parser.add_argument("--clippy-module", required=True, help=argparse.SUPPRESS)
    arguments = parser.parse_args()

    specification = importlib.util.spec_from_file_location("clippy", arguments.clippy_module)
    clippy = importlib.util.module_from_spec(specification)
    specification.loader.exec_module(clippy)

    check = arguments.check
    if not (arguments.lint or arguments.tests or arguments.plugin_tests):
        arguments.lint = arguments.tests = arguments.plugin_tests = True

    buck = os.path.abspath("scripts/buck")
    run = Run()

    def rustfmt():
        flags = ["--check"] if check else []
        return run.command([arguments.rustfmt, "--edition", "2024"] + flags + rust_files())

    if arguments.lint:
        run.step("rustfmt", rustfmt)
        run.step(
            "crates/fix-rust-source",
            lambda: run.command([arguments.fix_rust_source] + (["--check"] if check else [])),
        )

        def starlark():
            config = "buck/starlark_fmt.json"
            if not check:
                return run.command([arguments.starlark_fmt, "--config", config, "fmt"] + starlark_files())
            # starlark_fmt has no check mode and always exits zero; what says a
            # file would change is its diff subcommand writing one.
            unformatted = [
                path
                for path in starlark_files()
                if subprocess.run(
                    [arguments.starlark_fmt, "--config", config, "diff", path],
                    stdout=subprocess.PIPE,
                    stderr=subprocess.DEVNULL,
                ).stdout.strip()
            ]
            if unformatted:
                print("These files are not formatted. Run //:verify without --check.")
                for path in unformatted:
                    print("  " + path)
            return not unformatted

        run.step("starlark_fmt", starlark)

        # Every first-party target, tests included, in every configuration it
        # is built in: the host's and the plugins' and games' wasm.
        run.step("clippy", lambda: clippy.run(buck, fixing=not check))

    if arguments.tests:
        # The merge queue in .github/merge-queue is the one part of this
        # repository that is not Rust. Node is not otherwise needed, so a
        # checkout without it loses nothing but the early warning; CI runs
        # these in a job of their own where Node is always present.
        if shutil.which("node"):
            run.step(
                "merge queue tests",
                lambda: run.command(
                    ["node", "--test", "--test-reporter=dot"] + glob.glob(".github/merge-queue/*.test.js")
                ),
            )
        else:
            print("Skipping the merge queue tests: node is not installed.")

        # Everything but the plugin tests, which are labelled plugin. Most of
        # it runs on BuildBuddy's workers; guides/buck2.md says which stays.
        run.step("buck2 test", lambda: run.command([buck, "test", "//crates/...", "--exclude", "plugin"]))

    if arguments.plugin_tests:
        # The plugin tests run here rather than on a worker, and a changed
        # painting is not a failure to fix in a run that writes: the accepted
        # file is rewritten, and it is the diff a person reviews.
        accept = [] if check else ["--", "--env", "UPDATE_SNAPSHOTS=1"]
        run.step(
            "plugin tests",
            lambda: run.command([buck, "test", "//crates/...", "--include", "plugin"] + accept),
        )

    # In case one of the commands above left something unformatted.
    if arguments.lint:
        run.step("format one more time", rustfmt)

    print("")
    if run.failed:
        print("Verification failed.")
        sys.exit(1)
    print("All checks passed.")


main()

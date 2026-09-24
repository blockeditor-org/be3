# What `./scripts/buck run //:verify` runs: fix-rust-source, rustfmt,
# starlark_fmt, clippy, the tests and the plugin tests. By default every tool
# writes its fixes and the plugin tests accept new paintings; --check writes
# nothing, as CI's lint job wants, and --ci writes with CI's verbose output. The
# parts - --lint, --tests, --plugin-tests - need nothing from one another, so
# CI runs them on three runners; naming none runs all three.
#
# Usage:
#   ./scripts/buck run //:verify
#   ./scripts/buck run //:verify -- --check
#   ./scripts/buck run //:verify -- --tests --plugin-tests

import argparse
import glob
import os
import shutil
import subprocess
import sys
import time

import importlib.util

# Starlark files a person wrote. The generated ones are left out,
# because a formatter that rewrote them would put it and the check that they
# are current permanently at odds.
STARLARK_ROOTS = ["buck", "crates", "third-party/system", "third-party/pdfium", "BUCK.v2", "PACKAGE"]
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


LINT_TOOLS = {
    "rustfmt": ("//buck/tools:rustfmt-sysroot", "bin/rustfmt"),
    "starlark_fmt": ("//buck/tools:starlark_fmt", None),
    "fix-rust-source": ("//crates/fix-rust-source:fix-rust-source-bin", None),
}


def lint_tools(buck):
    # Built for this machine's buck2 configuration, which is where the
    # formatters run, and asked for in one command, so they come down side by
    # side. --show-full-output prints each target beside its absolute path.
    targets = [target for target, _ in LINT_TOOLS.values()]
    result = subprocess.run([buck, "build", "--show-full-output"] + targets, stdout=subprocess.PIPE, text=True)
    if result.returncode != 0:
        sys.exit("The lint pass's tools did not build.")
    printed = result.stdout
    built = {}
    for line in printed.splitlines():
        parts = line.split(" ", 1)
        if len(parts) == 2:
            built[parts[0]] = parts[1]
    paths = {}
    for name, (target, inside) in LINT_TOOLS.items():
        label = "root" + target
        if label not in built:
            sys.exit("buck2 printed no output for {}".format(target))
        paths[name] = os.path.join(built[label], inside) if inside else built[label]
    return paths


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

    # The lint pass's tools, built only when there is a lint pass: rustfmt and
    # starlark_fmt are downloads and fix-rust-source a build, and the tests
    # need none of them, so a run of the tests alone does not fetch them.
    tools = {}
    if arguments.lint:
        tools = lint_tools(buck)

    def rustfmt():
        flags = ["--check"] if check else []
        return run.command([tools["rustfmt"], "--edition", "2024"] + flags + rust_files())

    if arguments.lint:
        run.step("rustfmt", rustfmt)
        run.step(
            "crates/fix-rust-source",
            lambda: run.command([tools["fix-rust-source"]] + (["--check"] if check else [])),
        )

        def starlark():
            config = "buck/starlark_fmt.json"
            if not check:
                return run.command([tools["starlark_fmt"], "--config", config, "fmt"] + starlark_files())
            # starlark_fmt has no check mode and always exits zero; what says a
            # file would change is its diff subcommand writing one.
            unformatted = [
                path
                for path in starlark_files()
                if subprocess.run(
                    [tools["starlark_fmt"], "--config", config, "diff", path],
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
        # The merge queue's tests, when node is installed; CI runs them in a job of
        # their own.
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

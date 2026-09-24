#!/bin/sh
#
# What `./scripts/buck run //:verify` runs: fix-rust-source, rustfmt,
# starlark_fmt and clippy (--lint), the tests (--tests), and the plugin tests
# (--plugin-tests), which run here because they read and write snapshots/.
# Naming none runs all three; CI runs them on three runners. Every tool writes
# its fixes and the plugin tests accept new paintings, unless --check, which
# writes nothing and fails on anything that would change.
#
# Usage:
#   ./scripts/buck run //:verify [-- --check] [--lint] [--tests] [--plugin-tests]
set -u
buck="$(pwd)/scripts/buck"
check=false lint=false tests=false plugin_tests=false
for argument in "$@"; do
    case "$argument" in
        --check) check=true ;;
        --lint) lint=true ;;
        --tests) tests=true ;;
        --plugin-tests) plugin_tests=true ;;
        *) echo "Usage: ./scripts/buck run //:verify -- [--check] [--lint] [--tests] [--plugin-tests]" >&2; exit 1 ;;
    esac
done
if ! $lint && ! $tests && ! $plugin_tests; then
    lint=true tests=true plugin_tests=true
fi
failed=false

step() {
    name="$1"
    shift
    echo "$name..."
    started="$(date +%s)"
    "$@" || failed=true
    echo "  $name took $(($(date +%s) - started))s"
}

rust_files() {
    git ls-files --cached --others --exclude-standard -z -- 'crates/*.rs' | xargs -0 ls -d 2> /dev/null
}

starlark_files() {
    find buck crates third-party/system third-party/pdfium BUCK.v2 PACKAGE -type f \
        \( -name BUCK -o -name '*.bzl' -o -name '*.bxl' -o -name BUCK.v2 -o -name PACKAGE \) \
        ! -path buck/cargo/crates.bzl ! -path buck/sysroot/packages.bzl | sort
}

rustfmt() {
    if $check; then
        rust_files | xargs "$rustfmt" --edition 2024 --check
    else
        rust_files | xargs "$rustfmt" --edition 2024
    fi
}

starlark() {
    if ! $check; then
        starlark_files | xargs "$starlark_fmt" --config buck/starlark_fmt.json fmt
        return
    fi
    unformatted="$(starlark_files | while read -r file; do
        [ -n "$("$starlark_fmt" --config buck/starlark_fmt.json diff "$file" 2> /dev/null)" ] && echo "  $file"
    done)"
    [ -z "$unformatted" ] && return 0
    echo "These files are not formatted; run //:verify without --check:"
    echo "$unformatted"
    return 1
}

if $lint; then
    tools="$("$buck" build --show-full-output //buck/tools:rustfmt-sysroot //buck/tools:starlark_fmt \
        //crates/fix-rust-source:fix-rust-source-bin //crates/buck-tools:buck-tools-bin)" || exit 1
    path() { echo "$tools" | awk -v target="$1" '$1 == target { print $2 }'; }
    rustfmt="$(path root//buck/tools:rustfmt-sysroot)/bin/rustfmt"
    starlark_fmt="$(path root//buck/tools:starlark_fmt)"
    fix_rust_source="$(path root//crates/fix-rust-source:fix-rust-source-bin)"
    buck_tools="$(path root//crates/buck-tools:buck-tools-bin)"

    step rustfmt rustfmt
    if $check; then
        step fix-rust-source "$fix_rust_source" --check
        step clippy "$buck_tools" clippy "$buck"
    else
        step fix-rust-source "$fix_rust_source"
        step clippy "$buck_tools" clippy "$buck" --fix
    fi
    step starlark_fmt starlark
fi

if $tests; then
    if command -v node > /dev/null; then
        step "merge queue tests" sh -c 'node --test --test-reporter=dot .github/merge-queue/*.test.js'
    else
        echo "Skipping the merge queue tests: node is not installed."
    fi
    step "buck2 test" "$buck" test //crates/... --exclude plugin
fi

if $plugin_tests; then
    if $check; then
        step "plugin tests" "$buck" test //crates/... --include plugin
    else
        step "plugin tests" "$buck" test //crates/... --include plugin -- --env UPDATE_SNAPSHOTS=1
    fi
fi

if $lint; then
    step "format one more time" rustfmt
fi

echo
if $failed; then
    echo "Verification failed."
    exit 1
fi
echo "All checks passed."

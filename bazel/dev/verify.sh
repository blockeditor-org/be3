#!/bin/sh
#
# What `./scripts/bazel run //:verify` runs: fix-rust-source, rustfmt,
# buildifier and clippy (--lint), the tests (--tests), and the plugin tests
# (--plugin-tests), which run here because they read and write snapshots/.
# Naming none runs all three; CI runs them on three runners. Every tool writes
# its fixes and the plugin tests accept new paintings, unless --check, which
# writes nothing and fails on anything that would change.
#
# Usage:
#   ./scripts/bazel run //:verify [-- --check] [--lint] [--tests] [--plugin-tests]
set -u
cd "$BUILD_WORKSPACE_DIRECTORY"
bazel="$(pwd)/scripts/bazel"
check=false lint=false tests=false plugin_tests=false
for argument in "$@"; do
    case "$argument" in
        --check) check=true ;;
        --lint) lint=true ;;
        --tests) tests=true ;;
        --plugin-tests) plugin_tests=true ;;
        *) echo "Usage: ./scripts/bazel run //:verify -- [--check] [--lint] [--tests] [--plugin-tests]" >&2; exit 1 ;;
    esac
done
if ! $lint && ! $tests && ! $plugin_tests; then
    lint=true tests=true plugin_tests=true
fi
failed=false

# On a pull request, the lockfile Bazel keeps of what the module extensions
# made (MODULE.bazel.lock) is brought up to date like any other fix; anywhere
# else, one that is out of date fails the command that finds it.
if $check; then
    lockfile_mode=error
else
    lockfile_mode=update
fi

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

bazel_files() {
    git ls-files --cached --others --exclude-standard -- \
        'MODULE.bazel' '*BUILD.bazel' '*.bzl' \
        | grep -v '^bazel/sysroot/packages.bzl$' | while read -r file; do [ -e "$file" ] && echo "$file"; done
}

# The executable bit is part of an action's inputs, and a Windows checkout has
# none, so a file a build reads must not have one either, or Windows misses
# every cache entry Linux wrote. Only the scripts people run by hand keep it,
# and the credential helper Bazel runs itself.
file_modes() {
    executable="$(git ls-files -z | xargs -0 sh -c 'for file; do [ -f "$file" ] && [ -x "$file" ] && echo "$file"; done' sh \
        | grep -v -e '^scripts/[^/]*$' -e '^scripts/internal/install-bazel.sh$' -e '^scripts/internal/buildbuddy-credentials$')"
    [ -z "$executable" ] && return 0
    if ! $check; then
        echo "$executable" | while read -r file; do chmod -x "$file"; done
        return 0
    fi
    echo "These files are executable; run //:verify without --check:"
    echo "$executable" | sed 's/^/  /'
    return 1
}

rustfmt() {
    if $check; then
        rust_files | xargs "$rustfmt" --edition 2024 --check
    else
        rust_files | xargs "$rustfmt" --edition 2024
    fi
}

buildifier() {
    if $check; then
        bazel_files | xargs "$buildifier" -mode=check -lint=off || {
            echo "These files are not formatted; run //:verify without --check."
            return 1
        }
    else
        bazel_files | xargs "$buildifier" -mode=fix -lint=off
    fi
}

# Every plugin test names the painting it compares in USED_PAINTINGS, so once
# all of them pass, a painting none of them named belongs to a test that is
# gone: it is deleted, or with --check it fails the run. The directory is in
# the checkout because that is all a plugin test can write to. Every plugin
# test runs, rather than the ones whose inputs changed, so that each names its
# paintings.
used_paintings="$(pwd)/target/used-paintings"

plugin_tests() {
    rm -rf "$used_paintings"
    mkdir -p "$used_paintings"
    if $check; then
        "$bazel" test --lockfile_mode="$lockfile_mode" --test_tag_filters=plugin --nocache_test_results \
            --test_env="USED_PAINTINGS=$used_paintings" //crates/... || return 1
    else
        "$bazel" test --lockfile_mode="$lockfile_mode" --test_tag_filters=plugin --nocache_test_results \
            --test_env=UPDATE_SNAPSHOTS=1 --test_env="USED_PAINTINGS=$used_paintings" //crates/... || return 1
    fi
    unused="$(for painting in snapshots/*.paint; do
        [ -e "$painting" ] && [ ! -e "$used_paintings/${painting#snapshots/}" ] && echo "$painting"
    done)"
    [ -z "$unused" ] && return 0
    if ! $check; then
        echo "Deleting the paintings no test compared:"
        echo "$unused" | sed 's/^/  /'
        echo "$unused" | while read -r painting; do rm "$painting"; done
        return 0
    fi
    echo "No test compared these paintings; run //:verify without --check to delete them:"
    echo "$unused" | sed 's/^/  /'
    return 1
}

# A tool this machine runs, built by Bazel: the path of its one output.
tool() {
    "$bazel" build --lockfile_mode="$lockfile_mode" "$1" > /dev/null 2>&1 || "$bazel" build --lockfile_mode="$lockfile_mode" "$1" || return 1
    path="$("$bazel" cquery --lockfile_mode="$lockfile_mode" --output=files "$1" 2> /dev/null | head -n 1)"
    echo "$("$bazel" info execution_root 2> /dev/null)/$path"
}

if $lint; then
    rustfmt="$(tool //bazel/dev:rustfmt)" || exit 1
    buildifier="$(tool //bazel/tools:buildifier)" || exit 1
    fix_rust_source="$(tool //crates/fix-rust-source:fix-rust-source-bin)" || exit 1
    bazel_tools="$(tool //crates/bazel-tools:bazel-tools-bin)" || exit 1

    step "file modes" file_modes
    step rustfmt rustfmt
    if $check; then
        step fix-rust-source "$fix_rust_source" --check
        step clippy "$bazel_tools" clippy "$bazel" --lockfile_mode="$lockfile_mode"
    else
        step fix-rust-source "$fix_rust_source"
        step clippy "$bazel_tools" clippy "$bazel" --lockfile_mode="$lockfile_mode" --fix
    fi
    step buildifier buildifier
fi

if $tests; then
    if command -v node > /dev/null; then
        step "merge queue tests" sh -c 'node --test --test-reporter=dot .github/merge-queue/*.test.js'
    else
        echo "Skipping the merge queue tests: node is not installed."
    fi
    step "bazel test" "$bazel" test --lockfile_mode="$lockfile_mode" --test_tag_filters=-plugin //crates/...
fi

if $plugin_tests; then
    step "plugin tests" plugin_tests
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

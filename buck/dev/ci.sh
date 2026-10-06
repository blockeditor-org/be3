#!/bin/sh
#
# What CI runs, as one command: the workflows linted, the sysroot lockfile
# re-resolved, //:verify, the Android app and launcher CI ships, and the paint
# previews of what a pull request changed. Each part runs whether or not the one before it passed, and
# the command fails at the end if any did. --check is //:verify's: nothing is
# written, and a lockfile that is out of date fails. --android writes the two
# signed APKs into DIR. --previews renders every painting that changed between
# BASE's merge base and HEAD, as committed, into OUT, with the comment
# crates/paint-snapshot's preview example writes.
#
# Usage:
#   ./scripts/buck run //:ci -- [--check] [--android DIR] [--previews BASE OUT]
set -u
buck="$(pwd)/scripts/buck"
check=false android='' base='' previews=''
while [ $# -gt 0 ]; do
    case "$1" in
        --check) check=true; shift ;;
        --android) android="$2"; shift 2 ;;
        --previews) base="$2" previews="$3"; shift 3 ;;
        *) echo "Usage: ./scripts/buck run //:ci -- [--check] [--android DIR] [--previews BASE OUT]" >&2; exit 1 ;;
    esac
done
failed=false

step() {
    echo "$1..."
    shift
    "$@" || failed=true
}

# The paintings the pull request changed, before and after, read from git so
# that what //:verify accepts below is not mistaken for what was committed.
render_previews() {
    from="$(git merge-base "$base" HEAD)" || return 1
    before="$(pwd)/target/previews/before" after="$(pwd)/target/previews/after"
    rm -rf "$before" "$after"
    mkdir -p "$before" "$after"
    git diff --name-only --no-renames "$from" HEAD -- 'snapshots/*.paint' |
        while IFS= read -r path; do
            name="$(basename "$path")"
            if git cat-file -e "$from:$path" 2> /dev/null; then
                git show "$from:$path" > "$before/$name"
            fi
            if git cat-file -e "HEAD:$path" 2> /dev/null; then
                git show "HEAD:$path" > "$after/$name"
            fi
        done
    "$buck" run //crates/paint-snapshot:preview-example -- "$before" "$after" "$previews"
}

# buck/sysroot/packages.bzl is the lockfile of what buck/sysroot/BUCK asks for,
# and is checked in. Re-resolving it is what says the two still agree; without
# --check a stale one is a fix like any other.
lock_sysroot() {
    "$buck" run //:lock-sysroot || return 1
    if $check; then
        git diff --exit-code buck/sysroot/packages.bzl
    fi
}

# zizmor, with .github/zizmor.yml's policy. With GH_TOKEN set it also checks
# that each pinned commit is really in the repository it is attributed to;
# without it, only what reading the workflows can tell.
lint_workflows() {
    "$buck" run //buck/tools:zizmor -- --config .github/zizmor.yml .github/workflows .github/actions
}

if [ -n "$previews" ]; then
    step 'Rendering the paint previews' render_previews
fi
step 'Linting the workflows' lint_workflows
step 'Re-resolving the sysroot lockfile' lock_sysroot
if $check; then
    step 'Verifying' "$buck" run //:verify -- --check
else
    step 'Verifying' "$buck" run //:verify
fi
if [ -n "$android" ]; then
    step 'Building the Android app' "$buck" build //crates/block-app:android-dist --out "$android/block-app.apk"
    step 'Building the Android launcher' "$buck" build //crates/be-launcher:android-dist --out "$android/be-launcher.apk"
fi

if $failed; then
    echo 'CI failed.'
    exit 1
fi

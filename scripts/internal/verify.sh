#!/usr/bin/env bash
#
# What ./scripts/verify and ./scripts/ci run. One buck2 command,
# buck/dev/verify.bxl, builds everything: the generated rules and Cargo.lock,
# the sysroot's lockfile, the autofixes, the tests and the plugin tests, and
# for CI the Android builds and the paint preview renderer. This then reads
# what it built, without buck2: it writes the fixes, the generated files and
# the paintings that changed into the checkout, or with --check reports them
# and fails, and fails if anything did not build or pass.
#
# Locally the build prints only what failed, cut down by quiet.awk; in CI, or
# with --verbose, it prints everything as it goes.
#
# Usage: scripts/internal/verify.sh [--check] [--lint] [--tests] [--plugin-tests]
#   [--verbose] [--android DIR] [--previews BASE OUT]

set -uo pipefail

source "$(dirname "${BASH_SOURCE[0]}")/common.sh"
cd "$repository"

check=false lint=false tests=false plugin_tests=false verbose=false
android='' base='' previews=''
[[ -n "${CI:-}" && "${CI:-}" != false ]] && verbose=true
while [[ $# -gt 0 ]]; do
    case "$1" in
        --check) check=true; shift ;;
        --lint) lint=true; shift ;;
        --tests) tests=true; shift ;;
        --plugin-tests) plugin_tests=true; shift ;;
        --verbose) verbose=true; shift ;;
        --android) android="$2"; shift 2 ;;
        --previews) base="$2" previews="$3"; shift 3 ;;
        *) echo "unknown argument $1" >&2; exit 1 ;;
    esac
done
if ! $lint && ! $tests && ! $plugin_tests; then
    lint=true tests=true plugin_tests=true
fi
$verbose && export BE3_VERBOSE=1
failed=false

mkdir -p target
manifest="$repository/target/verify-manifest.txt"
log="$repository/target/verify.log"
bxl=(
    bxl --keep-going //buck/dev/verify.bxl:main --
    --lint "$lint" --tests "$tests" --plugin_tests "$plugin_tests"
    --android "$([[ -n "$android" ]] && echo true || echo false)"
    --previews "$([[ -n "$previews" ]] && echo true || echo false)"
)
if $verbose; then
    "$repository/scripts/buck" "${bxl[@]}" > "$manifest" || failed=true
elif ! "$repository/scripts/buck" "${bxl[@]}" > "$manifest" 2> "$log"; then
    failed=true
    awk -f "$internal/quiet.awk" < "$log"
fi

# What the build printed of one kind, a path a line, leaving out what it failed
# to build.
built() {
    awk -F '\t' -v kind="$1" '$1 == kind { print $2 }' "$manifest" | while IFS= read -r path; do
        [[ -e "$path" ]] && echo "$path"
    done
}

# Copies a file the build made over the checkout's copy, or with --check says
# that it differs.
update() {
    local from="$1" to="$2"
    cmp -s "$from" "$to" && return 0
    if $check; then
        echo "Out of date: $to"
        failed=true
        return 0
    fi
    if mkdir -p "$(dirname "$to")" && cp "$from" "$to.partial" && mv -f "$to.partial" "$to"; then
        echo "Updated $to"
    else
        echo "Could not update $to"
        failed=true
    fi
}

# The executable bit is part of an action's inputs, and a Windows checkout has
# none, so a file a build reads must not have one either, or Windows misses
# every cache entry Linux wrote. Only the scripts people run by hand, and the
# installers ./scripts/buck runs, keep it.
if $lint; then
    executable="$(git ls-files -z | xargs -0 sh -c 'for file; do [ -f "$file" ] && [ -x "$file" ] && echo "$file"; done' sh \
        | grep -v -e '^scripts/[^/]*$' -e '^scripts/internal/install-buck2.sh$' -e '^scripts/internal/install-nsc.sh$')"
    if [[ -n "$executable" ]]; then
        if $check; then
            echo "These files are executable; run ./scripts/verify without --check:"
            echo "$executable" | sed 's/^/  /'
            failed=true
        else
            echo "$executable" | while IFS= read -r file; do chmod -x "$file"; done
        fi
    fi
fi

# The generated rules and Cargo.lock (buck/cargo/update.sh), and the sysroot's
# lockfile (buck/sysroot/lock.sh). The build used the checked-in ones; if these
# differ, the run the fix starts builds with the new ones.
generated="$(built buckify)"
[[ -n "$generated" ]] && update "$generated/crates.bzl" buck/cargo/crates.bzl
[[ -n "$generated" ]] && update "$generated/Cargo.lock" Cargo.lock
lock="$(built sysroot)"
[[ -n "$lock" ]] && update "$lock" buck/sysroot/packages.bzl

# The autofixes (buck/dev/fix.sh): each changed/ copied over the checkout, each
# deleted file deleted, and the clippy findings without a fix printed.
if $lint; then
    fixes="$(built fix)"
    changed="$(echo "$fixes" | while IFS= read -r fix; do
        [[ -n "$fix" ]] && (cd "$fix/changed" && find . -type f | sed 's|^\./||')
    done | sort -u)"
    deleted="$(echo "$fixes" | while IFS= read -r fix; do
        [[ -n "$fix" ]] && cat "$fix/deleted"
    done | sort -u)"
    if [[ -n "$changed$deleted" ]] && $check; then
        echo "The autofixes would change these files; run ./scripts/verify without --check:"
        echo "$changed$deleted" | sed '/^$/d; s/^/  /'
        failed=true
    elif [[ -n "$changed$deleted" ]]; then
        echo "Fixed:"
        echo "$fixes" | {
            status=0
            while IFS= read -r fix; do
                while IFS= read -r path; do
                    if mkdir -p "$(dirname "$path")" && cp "$fix/changed/$path" "$path"; then
                        echo "  $path"
                    else
                        echo "  $path could not be written"
                        status=1
                    fi
                done < <(cd "$fix/changed" && find . -type f | sed 's|^\./||')
            done
            exit $status
        } || failed=true
        echo "$deleted" | {
            status=0
            while IFS= read -r path; do
                [[ -n "$path" ]] || continue
                if rm -f "$path"; then
                    echo "  $path (deleted)"
                else
                    echo "  $path could not be deleted"
                    status=1
                fi
            done
            exit $status
        } || failed=true
    fi
    findings="$(echo "$fixes" | while IFS= read -r fix; do
        [[ -n "$fix" ]] && ls "$fix/findings"
    done | sort -u)"
    if [[ -n "$findings" ]]; then
        echo "$findings" | while IFS= read -r name; do
            cat "$(echo "$fixes" | while IFS= read -r fix; do
                [[ -f "$fix/findings/$name" ]] && echo "$fix/findings/$name"
            done | head -n 1)"
        done
        echo "clippy: $(echo "$findings" | wc -l | tr -d ' ') findings."
        failed=true
    fi
fi

# Each plugin test accepts every painting into its output (buck/wasm/defs.bzl):
# changed/ holds the ones that changed or are new, with why beside each, and
# used/ names every painting it compared. Those are copied into snapshots/, or
# with --check fail the run. Once all of them pass, a painting none of them
# named belongs to a test that is gone: it is deleted, or with --check it fails
# the run.
if $plugin_tests; then
    expected="$(awk -F '\t' '$1 == "paintings"' "$manifest" | wc -l)"
    directories="$(built paintings)"
    changed="$(echo "$directories" | while IFS= read -r directory; do
        [[ -n "$directory" ]] || continue
        for painting in "$directory"/changed/*.paint; do
            [[ -e "$painting" ]] && echo "$painting"
        done
    done)"
    if [[ -n "$changed" ]] && $check; then
        echo "These paintings changed; run ./scripts/verify without --check to accept them, then review them in a Paint review block:"
        echo "$changed" | while IFS= read -r painting; do
            echo "  snapshots/$(basename "$painting"): $(cat "$painting.why")"
        done
        failed=true
    elif [[ -n "$changed" ]]; then
        echo "Accepting the paintings that changed:"
        echo "$changed" | {
            status=0
            while IFS= read -r painting; do
                echo "  snapshots/$(basename "$painting"): $(cat "$painting.why")"
                cp "$painting" snapshots/ || status=1
            done
            exit $status
        } || failed=true
    fi
    if [[ "$expected" -gt 0 && "$(echo "$directories" | grep -c .)" -eq "$expected" ]]; then
        used="$(echo "$directories" | while IFS= read -r directory; do ls "$directory/used"; done)"
        unused="$(for painting in snapshots/*.paint; do
            [[ -e "$painting" ]] && ! echo "$used" | grep -qxF "${painting#snapshots/}" && echo "$painting"
        done)"
        if [[ -n "$unused" ]] && $check; then
            echo "No test compared these paintings; run ./scripts/verify without --check to delete them:"
            echo "$unused" | sed 's/^/  /'
            failed=true
        elif [[ -n "$unused" ]]; then
            echo "Deleting the paintings no test compared:"
            echo "$unused" | sed 's/^/  /'
            echo "$unused" | {
                status=0
                while IFS= read -r painting; do rm "$painting" || status=1; done
                exit $status
            } || failed=true
        fi
    fi
fi

if [[ -n "$android" ]]; then
    mkdir -p "$android"
    for apk in block-app.apk be-launcher.apk; do
        path="$(built "$apk")"
        if [[ -n "$path" ]]; then
            cp "$path" "$android/$apk" || failed=true
        else
            failed=true
        fi
    done
fi

# The paintings the pull request changed, before and after, read from git so
# that what was accepted above is not mistaken for what was committed, and
# drawn into OUT with the comment crates/paint-snapshot's preview example
# writes, which also counts the lines the pull request added and removed
# outside its tests.
render_previews() {
    local renderer from before after path name added removed files
    renderer="$(built previews)"
    [[ -n "$renderer" ]] || return 1
    from="$(git merge-base "$base" HEAD)" || return 1
    before="$repository/target/previews/before" after="$repository/target/previews/after"
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
    read -r added removed files < <(git diff --numstat "$from" HEAD -- . \
        ':(exclude,glob)**/tests.rs' ':(exclude,glob)**/tests/**' \
        ':(exclude,glob)**/test.rs' ':(exclude,glob)**/test/**' |
        awk '$1 != "-" { added += $1; removed += $2; files++ }
            END { printf "%d %d %d\n", added, removed, files }') || return 1
    "$renderer" "$before" "$after" "$previews" "$added" "$removed" "$files"
}

if [[ -n "$previews" ]]; then
    render_previews || failed=true
fi

echo
if $failed; then
    echo "Verification failed."
    exit 1
fi
echo "All checks passed."

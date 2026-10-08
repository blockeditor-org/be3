#!/usr/bin/env bash
#
# What ./scripts/verify and ./scripts/ci run. One buck2 command,
# buck/dev/verify.bxl, builds everything: the generated rules and Cargo.lock,
# the sysroot's lockfile, the autofixes, the tests and the plugin tests, for CI
# the Android builds and the paint preview renderer, and crates/verify for this
# machine. crates/verify then reads what was built, without buck2: it writes
# the fixes, the generated files and the paintings that changed into the
# checkout, or with --check reports them and fails, and fails if anything did
# not build or pass. Only what has to happen before the build is here.
#
# Locally the build prints only what failed, cut down by quiet.awk; in CI, or
# with --verbose, it prints everything as it goes.
#
# Usage: scripts/internal/verify.sh [--check] [--lint] [--tests] [--plugin-tests]
#   [--verbose] [--android DIR] [--previews BASE OUT]

set -uo pipefail

source "$(dirname "${BASH_SOURCE[0]}")/common.sh"
cd "$repository"

lint=false tests=false plugin_tests=false android=false previews=false
verbose=false
[[ -n "${CI:-}" && "${CI:-}" != false ]] && verbose=true
arguments=()
for argument in "$@"; do
    case "$argument" in
        --lint) lint=true ;;
        --tests) tests=true ;;
        --plugin-tests) plugin_tests=true ;;
        --android) android=true ;;
        --previews) previews=true ;;
        --verbose) verbose=true; continue ;;
    esac
    arguments+=("$argument")
done
if ! $lint && ! $tests && ! $plugin_tests; then
    lint=true tests=true plugin_tests=true
    arguments+=(--lint --plugin-tests)
fi
$verbose && export BE3_VERBOSE=1

mkdir -p target
manifest="$repository/target/verify-manifest.txt"
log="$repository/target/verify.log"
bxl=(
    bxl --keep-going //buck/dev/verify.bxl:main --
    --host "$(host_target_platform || echo linux_x86_64)"
    --lint "$lint" --tests "$tests" --plugin_tests "$plugin_tests"
    --android "$android" --previews "$previews"
)
if $verbose; then
    "$repository/scripts/buck" "${bxl[@]}" > "$manifest" || arguments+=(--build-failed)
elif ! "$repository/scripts/buck" "${bxl[@]}" > "$manifest" 2> "$log"; then
    arguments+=(--build-failed)
    awk -f "$internal/quiet.awk" < "$log"
fi

verify=''
while IFS=$'\t' read -r kind path; do
    [[ "$kind" == verify ]] && verify="${path%$'\r'}"
done < "$manifest"
if [[ -z "$verify" || ! -e "$verify" ]]; then
    echo
    echo 'Verification failed: crates/verify did not build.'
    exit 1
fi
exec "$verify" "$manifest" ${arguments[@]+"${arguments[@]}"}

#!/usr/bin/env bash

set -euo pipefail

source "$(dirname "${BASH_SOURCE[0]}")/common.sh"

triple=''
output=''
while [[ $# -gt 0 ]]; do
    case "$1" in
        --triple)
            triple="$2"
            shift 2
            ;;
        --output)
            output="$2"
            shift 2
            ;;
        *)
            echo "Unknown argument: $1" >&2
            exit 1
            ;;
    esac
done

assert_command curl 'Install curl.'
assert_command tar 'Install tar.'

if [[ -z "$triple" || -z "$output" ]]; then
    echo 'Usage: fetch-pdfium.sh --triple TRIPLE --output DIRECTORY' >&2
    exit 1
fi

# Pinned to a release rather than /latest/, so a build today and a build a
# year from now fetch the same bytes, and those bytes are checked against a
# hash recorded here rather than trusted from the download alone. The hashes
# were cross-checked against pdfium-binaries' own GitHub Actions build
# provenance attestation (pdfium-attestation.json in the release), not just
# recomputed from the same download.
pdfium_release='chromium/8044'
case "$triple" in
    x86_64-unknown-linux-gnu)
        asset='pdfium-linux-x64'
        library_path='lib/libpdfium.so'
        sha256='eb142f416aed3a72fc5a02dbd5884868a16cb99dc0cf53e6bdd64afbf67b05f4'
        ;;
    aarch64-unknown-linux-gnu)
        asset='pdfium-linux-arm64'
        library_path='lib/libpdfium.so'
        sha256='e98400ef5f005f27cfba5c14f72d464e25187298f04950de46646033cf24cef0'
        ;;
    x86_64-apple-darwin)
        asset='pdfium-mac-x64'
        library_path='lib/libpdfium.dylib'
        sha256='a93d44238e05de20028446561b951d50988b849efbbe56fe40c0d376c05b45e8'
        ;;
    aarch64-apple-darwin)
        asset='pdfium-mac-arm64'
        library_path='lib/libpdfium.dylib'
        sha256='61424884d4a7f153b808deba6437848e4400834ce30aaf95d3050da44df8f420'
        ;;
    x86_64-pc-windows-msvc|x86_64-pc-windows-gnu)
        asset='pdfium-win-x64'
        library_path='bin/pdfium.dll'
        sha256='78a17d9a5f14467631c26a3ac8741b27a0471ecc05bd6a119b523598160a0537'
        ;;
    aarch64-pc-windows-msvc|aarch64-pc-windows-gnu)
        asset='pdfium-win-arm64'
        library_path='bin/pdfium.dll'
        sha256='6c9ac0ddc69edd8a18d47b95098a5b843eaed5c5bbdcb9587a18c196457449f8'
        ;;
    *)
        echo "No PDFium binary is known for target $triple" >&2
        exit 1
        ;;
esac
library_name="$(basename "$library_path")"

# macOS has shasum rather than sha256sum; both read a "hash  path" pair from
# stdin in the same format, so which one is picked only changes the command.
hash_of() {
    if command -v sha256sum > /dev/null; then
        sha256sum "$1" | cut -d ' ' -f 1
    else
        assert_command shasum 'Install coreutils (sha256sum) or shasum.'
        shasum -a 256 "$1" | cut -d ' ' -f 1
    fi
}

mkdir -p "$output"
destination="$output/$library_name"
if [[ -f "$destination" ]]; then
    echo "PDFium is already present at $destination"
    exit 0
fi

tools_directory="$repository/target/tools"
extract_directory="$tools_directory/$asset"
# CI persists this extraction across runs (see the "Cache PDFium" step in
# ci.yml), since the release download is the flaky part: the pinned URL
# still goes through a release-assets redirect that intermittently 504s.
if [[ ! -f "$extract_directory/$library_path" ]]; then
    mkdir -p "$tools_directory"
    archive="$tools_directory/$asset.tgz"
    url="https://github.com/bblanchon/pdfium-binaries/releases/download/$pdfium_release/$asset.tgz"
    echo "Downloading PDFium from $url..."
    curl --fail --location --output "$archive" "$url"

    actual_sha256="$(hash_of "$archive")"
    if [[ "$actual_sha256" != "$sha256" ]]; then
        rm -f "$archive"
        echo "$asset.tgz has sha256 $actual_sha256, expected $sha256" >&2
        exit 1
    fi

    rm -rf "$extract_directory"
    mkdir -p "$extract_directory"
    tar -xzf "$archive" -C "$extract_directory"
    rm "$archive"
fi

cp "$extract_directory/$library_path" "$destination"
echo "Installed PDFium at $destination"
echo "PDFium is distributed under the licenses listed in $extract_directory/LICENSE"

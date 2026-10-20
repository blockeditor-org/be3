# The downloads every script makes: each pinned to the bytes it must be, and
# tried from a mirror when upstream will not answer, for build-ghostty-vt.sh,
# which runs on a worker.

repository="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
internal="$repository/scripts/internal"

assert_command() {
    if ! command -v "$1" > /dev/null; then
        echo "$1 was not found on PATH. $2" >&2
        exit 1
    fi
}

# macOS has shasum rather than sha256sum; both read a "hash  path" pair in the
# same format, so which one is picked only changes the command.
sha256_of() {
    if command -v sha256sum > /dev/null; then
        sha256sum "$1" | cut -d ' ' -f 1
    else
        assert_command shasum 'Install coreutils (sha256sum) or shasum.'
        shasum -a 256 "$1" | cut -d ' ' -f 1
    fi
}

# Where this repository keeps its own copies of the archives it downloads. A
# GitHub release asset is handed out through a redirect that intermittently
# answers 504, and a run lost that way is a run lost to something the change
# under test had no part in, so an archive that has cost us one is uploaded
# here as well and tried when upstream will not answer. The bytes are checked
# against the hash pinned beside the URL either way, which is what makes a
# mirror only another route to the same file rather than a second thing to
# trust.
#
# Filling one in is uploading the exact bytes of the upstream archive under the
# name the caller asks for. The names are not derived from upstream's, because
# upstream's are not always unique across releases, and a mirror still serving
# the last release's file would fail the hash check rather than the download.
download_mirror='https://lfs.pfg.pw/by-name/be3'

# One https download, with nothing checked about what arrived. Any further
# arguments go to curl after these, where they override them.
#
# --proto and --proto-redir together keep both the request and anything it is
# redirected to on https, so a redirect cannot quietly downgrade the transport.
# --retry covers the answers a host gives when it is briefly unhappy, 504
# among them, and leaves a 404 to fail at once, so a mirror nobody has filled
# in yet costs one request rather than five. curl waits between attempts on its
# own, doubling from a second. --speed-limit turns a transfer that has stalled
# into another of those attempts, which is what a 504 arriving after ten
# seconds of no bytes is.
fetch_https() {
    local destination="$1" source="$2"
    shift 2
    curl --fail --location --proto '=https' --proto-redir '=https' \
        --connect-timeout 30 --speed-limit 1024 --speed-time 60 \
        --retry 4 --retry-connrefused \
        "$@" --output "$destination" "$source"
}

# Everything this repository downloads goes through here, so that a pinned URL
# is also a pinned set of bytes. A version in a URL only says what we asked
# for; the hash is what says we got it. Without one, a moved release asset, a
# compromised host or a proxy that rewrites a response is a toolchain nobody
# looked at, running with whatever rights the build has - and a build fetches a
# compiler, a compiler wrapper and a sysroot, all of which end up in what ships.
#
# Any further arguments are mirrors of the same archive, tried in turn when the
# one before them cannot be downloaded at all. A file that arrives and fails
# the hash check stops the build instead: at that point one of the sources is
# serving something other than what is pinned, and moving on to the next would
# bury that.
#
# A mismatch deletes the download before exiting, so a retry fetches afresh
# instead of finding the rejected file already in place and extracting it.
download_verified() {
    local url="$1" destination="$2" expected="$3"
    shift 3
    assert_command curl 'Install curl.'
    local sources=("$url" "$@")
    local source actual
    for source in "${sources[@]}"; do
        rm -f "$destination"
        if fetch_https "$destination" "$source"; then
            actual="$(sha256_of "$destination")"
            if [[ "$actual" == "$expected" ]]; then
                return 0
            fi

            rm -f "$destination"
            echo '' >&2
            echo "The download from $source is not the file this repository expects." >&2
            echo "  sha256 is   $actual" >&2
            echo "  sha256 want $expected" >&2
            echo '' >&2
            echo 'Nothing has been installed. If the release was legitimately replaced, check the' >&2
            echo "new bytes against upstream's own provenance before recording the new hash here;" >&2
            echo 'recomputing it from the same download only proves the download agrees with' >&2
            echo 'itself.' >&2
            exit 1
        fi
        echo "The download from $source did not complete." >&2
    done

    rm -f "$destination"
    echo '' >&2
    echo "Nothing could be downloaded for $destination. These were tried:" >&2
    for source in "${sources[@]}"; do
        echo "  $source" >&2
    done
    exit 1
}

# download_verified for an archive that community mirrors carry and that the
# upstream host would rather not serve to automation: each mirror is tried in
# turn, and upstream is only the last resort. The mirrors are nobody this
# repository trusts, which the pinned hash is what makes safe, so unlike
# download_verified a mirror whose bytes do not match is reported and skipped
# rather than stopping the build; upstream's answer is still held to the full
# check. A mirror gets one short attempt, since the next one is the better
# retry: some are down, some hang, and some answer 429 to share their bandwidth.
download_verified_from_mirrors() {
    local url="$1" destination="$2" expected="$3"
    shift 3
    assert_command curl 'Install curl.'
    local mirror actual
    for mirror in "$@"; do
        rm -f "$destination"
        if ! fetch_https "$destination" "$mirror" \
            --retry 0 --connect-timeout 10 --speed-limit 65536 --speed-time 20; then
            echo "The download from $mirror did not complete." >&2
            continue
        fi
        actual="$(sha256_of "$destination")"
        if [[ "$actual" == "$expected" ]]; then
            return 0
        fi
        echo "The download from $mirror is not the file this repository expects (sha256 $actual); skipping it." >&2
    done
    download_verified "$url" "$destination" "$expected"
}

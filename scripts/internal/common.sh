repository="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
internal="$repository/scripts/internal"

# How long everything took.
#
# A script is a handful of steps of wildly different cost, and nothing in the
# output says which one a slow run was spent in. So each step announces itself
# through step, what it cost is reported when the next step begins or when the
# script ends, and the script reports its own total.
#
# Bash 5 keeps a sub-second clock in EPOCHREALTIME and GNU date has %N, but
# macOS ships neither with the bash it has, so a machine with only whole
# seconds is measured in whole seconds rather than shown the literal N its date
# would have printed.
clock='seconds'
if [[ -n "${EPOCHREALTIME:-}" ]]; then
    clock='bash'
elif [[ "$(date +%N 2> /dev/null)" =~ ^[0-9]+$ ]]; then
    clock='date'
fi

now_ms() {
    local now
    case "$clock" in
        bash)
            # A locale that writes the fraction after a comma still has to
            # arrive at the arithmetic below as a number bash will divide.
            now="${EPOCHREALTIME/,/.}"
            echo "$(( ${now%.*} * 1000 + 10#${now#*.} / 1000 ))"
            ;;
        date)
            echo "$(( $(date +%s%N) / 1000000 ))"
            ;;
        *)
            echo "$(( $(date +%s) * 1000 ))"
            ;;
    esac
}

format_duration() {
    local total="$1"
    if [[ "$total" -ge 60000 ]]; then
        printf '%dm %02ds' "$((total / 60000))" "$(((total % 60000) / 1000))"
    else
        printf '%d.%01ds' "$((total / 1000))" "$(((total % 1000) / 100))"
    fi
}

step_name=''
step_started=0

# Announces a step and starts its clock. The step before it ends here, so a
# script reads as the list of steps it already announced rather than as pairs
# of calls, and a step a shared function starts closes the caller's.
step() {
    end_step
    step_name="$1"
    step_started="$(now_ms)"
    echo "$step_name..."
}

end_step() {
    [[ -n "$step_name" ]] || return 0
    local name="$step_name"
    step_name=''
    echo "  $name took $(format_duration "$(($(now_ms) - step_started))")"
}

script_started=''
script_name=''

# The total a script ends with. It is an exit trap rather than a last line so
# that a script killed by a failing step still says how long it spent getting
# there, and so the step that failed is closed rather than left hanging.
time_script() {
    script_name="$1"
    script_started="$(now_ms)"
    trap report_total EXIT
}

report_total() {
    end_step
    [[ -n "$script_started" ]] || return 0
    echo "$script_name took $(format_duration "$(($(now_ms) - script_started))")"
    script_started=''
}

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
#
# --proto and --proto-redir together keep both the request and anything it is
# redirected to on https, so a redirect cannot quietly downgrade the transport.
download_verified() {
    local url="$1" destination="$2" expected="$3"
    shift 3
    assert_command curl 'Install curl.'
    local sources=("$url" "$@")
    local source actual
    for source in "${sources[@]}"; do
        rm -f "$destination"
        # --retry covers the answers a host gives when it is briefly unhappy,
        # 504 among them, and leaves a 404 to fail at once, so a mirror nobody
        # has filled in yet costs one request rather than five. curl waits
        # between attempts on its own, doubling from a second. --speed-limit
        # turns a transfer that has stalled into another of those attempts,
        # which is what a 504 arriving after ten seconds of no bytes is.
        if curl --fail --location --proto '=https' --proto-redir '=https' \
            --connect-timeout 30 --speed-limit 1024 --speed-time 60 \
            --retry 4 --retry-connrefused \
            --output "$destination" "$source"; then
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

# The buck2 release this repository is built with, and the reindeer that
# generates its third-party rules.
#
# buck2 is a prebuilt release, checked against the hashes below the way every
# other tool here is. A buck2 binary carries the prelude it was built with, so
# this version pins the build rules too, which is why it is the one string that
# has to move for a prelude change to arrive - and why moving it is a change to
# verify with a build rather than a number to bump.
#
# The tag is a dated release. "latest" is a tag upstream repoints on every push
# to main, so it is never what to pin.
#
# reindeer publishes no releases at all, so it is pinned by commit and built
# from source. Nothing but ./scripts/buck run //:buckify needs it: the file it writes is
# checked in.
buck2_version='2026-09-15'

# The releases mirrored for this version, by the triple upstream names them
# after. Linux and macOS are what anyone develops on; Windows is here because
# the release exists and mirroring it costs nothing, not because anything has
# been built there.
buck2_sha256_x86_64_unknown_linux_gnu='1268fce33fb61273091dd5dd8c60b65de1521a3caa36b88983813e882b7ffd89'
buck2_sha256_aarch64_unknown_linux_gnu='e35027a8e3f702fd9f080074ac64ebc80e2a0fe8d669d46d85afc31b0ec4a463'
buck2_sha256_x86_64_apple_darwin='af1e51f198ecccfc41b9d960a2c79bb16f01e238f7169a86989caed9440fba0d'
buck2_sha256_aarch64_apple_darwin='aacdf7cabe34b9b5dc74866b8dcf7410cdbae3ad2dccef45b7ee9851cb781528'
buck2_sha256_x86_64_pc_windows_msvc='1f0619b285ee3cbdc562a38b70f2636f73e8233b2f5156aef93e9e8a00079e2c'
buck2_sha256_aarch64_pc_windows_msvc='58edb0718e3b89a3f875129cc72640f2f4cb80487ec42057eb88447519dff0cf'

# The triple the buck2 release is named after, for this machine.
buck2_triple() {
    local architecture
    case "$(uname -m)" in
        x86_64 | amd64) architecture='x86_64' ;;
        aarch64 | arm64) architecture='aarch64' ;;
        *)
            echo "No buck2 release is mirrored for $(uname -m)." >&2
            return 1
            ;;
    esac

    case "$(uname -s)" in
        Linux) echo "$architecture-unknown-linux-gnu" ;;
        Darwin) echo "$architecture-apple-darwin" ;;
        MINGW* | MSYS* | CYGWIN*) echo "$architecture-pc-windows-msvc" ;;
        *)
            echo "No buck2 release is mirrored for $(uname -s)." >&2
            return 1
            ;;
    esac
}

# A path as a native Windows program can open it. Git Bash hands out /c/...
# paths, which Windows' own programs - Python among them - cannot read, so a
# path that travels to one as an argument goes through here. Anywhere else it
# is the path as it is.
native_path() {
    if command -v cygpath > /dev/null 2>&1; then
        cygpath -m "$1"
    else
        echo "$1"
    fi
}

# Where the pinned buck2 is installed: inside the checkout, under a directory
# named after the version, so ./scripts/buck can tell it is there with one test
# of a file, and moving the version installs the new one rather than running
# the old.
buck2_path() {
    echo "$repository/target/tools/buck2-$buck2_version/buck2$(buck2_binary_suffix)"
}

# The target platform in buck/platforms for this machine, for what
# ./scripts/buck runs here. Builds are for Linux on x86_64 wherever they are
# asked for; something that is to run on this machine is built for it instead.
host_target_platform() {
    local architecture
    case "$(uname -m)" in
        x86_64 | amd64) architecture='x86_64' ;;
        aarch64 | arm64) architecture='arm64' ;;
        *) return 1 ;;
    esac
    case "$(uname -s)" in
        Linux) echo "linux_$architecture" ;;
        Darwin) echo "macos_$architecture" ;;
        MINGW* | MSYS* | CYGWIN*) echo "windows_$architecture" ;;
        *) return 1 ;;
    esac
}

# Upstream names the Windows binaries .exe.zst and everything else .zst.
buck2_release_suffix() {
    case "$(uname -s)" in
        MINGW* | MSYS* | CYGWIN*) echo '.exe.zst' ;;
        *) echo '.zst' ;;
    esac
}

# What the installed binary is called, which on Windows keeps the extension the
# release was named with.
buck2_binary_suffix() {
    case "$(uname -s)" in
        MINGW* | MSYS* | CYGWIN*) echo '.exe' ;;
        *) echo '' ;;
    esac
}

# The hash pinned above for a tool on this machine, looked up by the triple with
# its dashes turned into the underscores a shell variable name can hold.
buck2_release_sha256() {
    local tool="$1" triple="$2" name
    name="${tool}_sha256_${triple//-/_}"
    if [[ -z "${!name:-}" ]]; then
        echo "No $tool release is pinned here for $triple." >&2
        return 1
    fi
    echo "${!name}"
}

# Every action runs on BuildBuddy, so a build without the key has nowhere to
# run. buck2 would say so only as a failed connection, well into the build. The
# key is BUILDBUDDY_API_KEY from the environment, or else the first of two files
# that holds it: .buildbuddy-api-key at the root of the checkout, which git
# ignores, or ~/.config/be3/buildbuddy-api-key. Either way it is exported, which
# is how .buckconfig's $BUILDBUDDY_API_KEY reaches the daemon.
#
# With none of them, a person at a terminal is asked for the key, and it is
# saved to .buildbuddy-api-key for next time. Anything else - a pipe, CI, an
# agent's shell - has nobody to answer, so it is told what to set instead of
# waiting for input that will not come.
assert_buildbuddy_key() {
    local file key
    if [[ -z "${BUILDBUDDY_API_KEY:-}" ]]; then
        for file in "$repository/.buildbuddy-api-key" "${XDG_CONFIG_HOME:-$HOME/.config}/be3/buildbuddy-api-key"; do
            if [[ -f "$file" ]]; then
                BUILDBUDDY_API_KEY="$(tr -d '[:space:]' < "$file")"
                break
            fi
        done
    fi
    if [[ -z "${BUILDBUDDY_API_KEY:-}" && -t 0 && -t 2 ]]; then
        echo 'buck2 runs every build on BuildBuddy, and needs an API key for it.' >&2
        echo 'Find one under Settings at https://app.buildbuddy.io.' >&2
        read -r -s -p 'BuildBuddy API key: ' key
        echo '' >&2
        key="$(printf '%s' "$key" | tr -d '[:space:]')"
        if [[ -n "$key" ]]; then
            (umask 077 && printf '%s\n' "$key" > "$repository/.buildbuddy-api-key")
            echo "Saved it to $repository/.buildbuddy-api-key, which git ignores." >&2
            BUILDBUDDY_API_KEY="$key"
        fi
    fi
    if [[ -n "${BUILDBUDDY_API_KEY:-}" ]]; then
        export BUILDBUDDY_API_KEY
        return 0
    fi
    echo 'buck2 runs every build on BuildBuddy, and there is no BuildBuddy API key.' >&2
    echo 'Set BUILDBUDDY_API_KEY, or write the key to .buildbuddy-api-key at the root' >&2
    echo 'of the checkout or to ~/.config/be3/buildbuddy-api-key. Run ./scripts/buck' >&2
    echo 'from a terminal to be asked for it. guides/buck2.md has more.' >&2
    exit 1
}

# An older ./scripts/buck wrote .buckconfig.local, and one left behind would
# point buck2 at a cache that is gone. A file a person wrote is left alone.
remove_generated_buck_config_local() {
    local path="$repository/.buckconfig.local"
    if [[ -f "$path" ]] && head -1 "$path" | grep -q '@generated by ./scripts/buck'; then
        rm "$path"
    fi
}


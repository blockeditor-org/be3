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

# The buck2 release this repository is built with.
#
# buck2 is a prebuilt release, checked against the hashes below the way every
# other tool here is. A buck2 binary carries the prelude it was built with, so
# this version pins the build rules too, which is why it is the one string that
# has to move for a prelude change to arrive - and why moving it is a change to
# verify with a build rather than a number to bump.
#
# The tag is a dated release. "latest" is a tag upstream repoints on every push
# to main, so it is never what to pin.
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

# The nsc release ./scripts/buck runs: Namespace's command-line tool, which
# wakes the remote execution cluster and names its hosts
# (guides/build_server.md). Its archives are checked against these hashes, by
# the platform upstream names them after.
nsc_version='0.0.586'
nsc_sha256_linux_amd64='57ba36c1b8dc02689c7d4f630ed9767d26f0eb98acaa56a50c15f30fe45f8f73'
nsc_sha256_linux_arm64='aa69e23a88df9ceb588bcbbe8781c08971d2b3ef40dbd89a3e68d6bdd361493c'
nsc_sha256_darwin_amd64='62814ca37432d7727928c2d5b0beeebc4cc012bdf474c122418146805688c7f6'
nsc_sha256_darwin_arm64='1e05852ce241508f4ced204688be434fe205a4d6bc382fddad114b95ac37abe9'
nsc_sha256_windows_amd64='6fb562526757d7cf4ad43d64980cded6b875bbf55667d7bfbb96a142127027e9'
nsc_sha256_windows_arm64='3b1b7d0fedd0ea07662bb6ea2894d3feb5bce747270694af563b5f8d9349c952'

nsc_platform() {
    local os architecture
    case "$(uname -s)" in
        Linux) os='linux' ;;
        Darwin) os='darwin' ;;
        MINGW* | MSYS* | CYGWIN*) os='windows' ;;
        *)
            echo "No nsc release is pinned here for $(uname -s)." >&2
            return 1
            ;;
    esac
    case "$(uname -m)" in
        x86_64 | amd64) architecture='amd64' ;;
        aarch64 | arm64) architecture='arm64' ;;
        *)
            echo "No nsc release is pinned here for $(uname -m)." >&2
            return 1
            ;;
    esac
    echo "${os}_$architecture"
}

nsc_path() {
    echo "$repository/target/tools/nsc-$nsc_version/nsc$(buck2_binary_suffix)"
}

# What buck2 reads the workspace's Cargo.toml files through -
# buck/cargo/crates.bzl, from which buck/cargo's macros write the rules for
# every workspace and third-party crate - is generated rather than checked in.
# buck/cargo/buckify.bxl makes it on a worker from the manifests, Cargo.lock
# and the layout of crates/, so everyone on the same Cargo.lock shares one cache
# entry: about two seconds on a fresh checkout, and a minute or so the first
# time anyone builds a new dependency.
#
# What decides whether to run it is a hash of those same inputs, kept beside
# the files; most runs only compare it. The files are written only when they
# change, so buck2 does not re-read them for nothing. target/Cargo.lock is the
# lockfile they were generated from, brought up to date with the manifests,
# which //:verify's lint copies over a stale Cargo.lock.
generated_rules=('buck/cargo/crates.bzl' 'target/Cargo.lock')

generated_rules_inputs() {
    (
        cd "$repository"
        printf '%s\n' "$buck2_version"
        find crates -type f | LC_ALL=C sort
        {
            printf '%s\0' Cargo.toml Cargo.lock buck/cargo/BUCK buck/cargo/buckify.bxl buck/tools/BUCK
            find crates -name Cargo.toml -print0
            find crates/buck-tools/src -type f -print0
        } | LC_ALL=C sort -z | xargs -0 cat
    ) | sha256sum | cut -d ' ' -f 1
}

ensure_generated_rules() {
    local buck2="$1" stamp="$repository/target/generated-rules.sha256" fingerprint path generated
    fingerprint="$(generated_rules_inputs)"
    local current=true
    for path in "${generated_rules[@]}"; do
        [[ -f "$repository/$path" ]] || current=false
    done
    if $current && [[ "$(cat "$stamp" 2> /dev/null)" == "$fingerprint" ]]; then
        return 0
    fi
    echo 'Generating the rules for the workspace'"'"'s crates...' >&2
    if generated="$("$buck2" bxl //buck/cargo/buckify.bxl:main 2> "$repository/target/generated-rules.log")"; then
        generated="$(printf '%s\n' "$generated" | tail -n 1)"
    else
        generated=''
    fi
    if [[ -z "$generated" || ! -f "$generated/crates.bzl" || ! -f "$generated/Cargo.lock" ]]; then
        cat "$repository/target/generated-rules.log" >&2
        echo 'Generating the rules for the workspace'"'"'s crates failed.' >&2
        exit 1
    fi
    write_if_changed "$generated/crates.bzl" "$repository/buck/cargo/crates.bzl"
    write_if_changed "$generated/Cargo.lock" "$repository/target/Cargo.lock"
    printf '%s\n' "$fingerprint" > "$stamp"
}

write_if_changed() {
    if ! cmp -s "$1" "$2"; then
        cp "$1" "$2.partial"
        mv -f "$2.partial" "$2"
    fi
}

# Where buck2 reaches the build server, written into .buckconfig.local.
# BE3_BUILD_SERVER picks the server: namespace, which is the default and CI's,
# or one of our own, blocks.pfg.pw and buildserver.pfg.pw, or buildbuddy. Each
# has its own key and its own cache. The file also names the server under
# [be3] build_server, since the workers' image is not the same on all of them
# (buck/tools/defs.bzl).
#
# Namespace's cluster shuts down when it has had nothing to do for a while, so
# its hosts are asked of nsc rather than kept anywhere:
# `nsc reapi setup buck2` starts the cluster if it is down and names its
# executor, its storage and the header that authenticates to them. It is
# always asked with the same arguments and the same --key: Namespace starts a
# new cluster beside the old one when either differs. That answer is kept in
# target/namespace/ and asked again when the executor stops answering, when it
# is more than three hours old, or when the token changes. nsc is only
# downloaded for a build on Namespace.
#
# nsc authenticates with Namespace's token: BE3_NAMESPACE_TOKEN from the
# environment, which is what CI and agents' sessions set, or else
# .namespace-token.json at the root of the checkout, which git ignores, or
# ~/.config/be3/namespace-token.json. Either holds what
# `nsc reapi create-token` writes, or the bare token. The other servers' keys
# are BE3_BUILD_SERVER_KEY, or else .build-server-key.SERVER at the root of the
# checkout or ~/.config/be3/build-server-key.SERVER. With none of them, a
# person at a terminal is asked for it, and it is saved under ~/.config/be3 for
# every checkout; anything else - a pipe, CI, an agent's shell - is told what
# to set instead.
#
# buck2's remote execution client never reads HTTPS_PROXY. Where the machine's
# way out is an HTTPS proxy, buck2 is pointed at scripts/internal/re-relay
# instead: a relay on localhost for each host, built with Go and left running
# in the background, that sends each call on through the proxy. Namespace's
# executor and storage are separate hosts, so there are two relays.
#
# A .buckconfig.local a person wrote is left alone. buck2 reads the remote
# execution settings when its daemon starts and remembers which blobs it has
# uploaded, so the daemon is stopped whenever the file changes; the file names
# the hosts even when the relays' addresses are all buck2 reads from it, so
# that a change of host behind a relay is a change to the file too.
re_relay_engine_address='127.0.0.1:18980'
re_relay_storage_address='127.0.0.1:18981'
namespace_directory="$repository/target/namespace"
namespace_cluster_key='buildserver'

# Whether to look at the executor even when nsc was asked moments ago: after a
# command failed with an infrastructure error, the cluster may have gone.
namespace_recheck=false

# The token file nsc is given, or nothing when there is none.
namespace_token_file() {
    local key saved="${XDG_CONFIG_HOME:-$HOME/.config}/be3/namespace-token.json"
    if [[ -n "${BE3_NAMESPACE_TOKEN:-}" ]]; then
        local file="$namespace_directory/token.json" token
        token="$(printf '%s' "$BE3_NAMESPACE_TOKEN" | tr -d '\r\n')"
        mkdir -p "$namespace_directory"
        if [[ "$token" != '{'* ]]; then
            token="{\"bearer_token\":\"$token\"}"
        fi
        (umask 077 && printf '%s\n' "$token" > "$file.partial")
        write_if_changed "$file.partial" "$file"
        rm -f "$file.partial"
        echo "$file"
    elif [[ -f "$repository/.namespace-token.json" ]]; then
        echo "$repository/.namespace-token.json"
    elif [[ -f "$saved" ]]; then
        echo "$saved"
    elif key="$(ask_for_build_server_key 'Namespace' 'token' "$saved")" && [[ -n "$key" ]]; then
        if [[ "$key" != '{'* ]]; then
            (umask 077 && printf '{"bearer_token":"%s"}\n' "$key" > "$saved")
        fi
        echo "$saved"
    fi
}

# The server BE3_BUILD_SERVER names, namespace when it names none.
build_server() {
    local server="${BE3_BUILD_SERVER:-namespace}"
    case "$server" in
        namespace | blocks.pfg.pw | buildserver.pfg.pw | buildbuddy) echo "$server" ;;
        *)
            echo "BE3_BUILD_SERVER is $server, which is none of namespace, blocks.pfg.pw," >&2
            echo 'buildserver.pfg.pw and buildbuddy.' >&2
            return 1
            ;;
    esac
}

# Asks a person at a terminal for a server's key and saves it to the file
# given, printing it. Without a terminal it prints nothing.
ask_for_build_server_key() {
    local server="$1" what="$2" saved="$3" key
    [[ -t 0 && -t 2 ]] || return 0
    echo "buck2 builds on $server, and needs its $what." >&2
    echo 'guides/build_server.md says where to get one.' >&2
    read -r -s -p "$server $what: " key
    echo '' >&2
    key="$(printf '%s' "$key" | tr -d '[:space:]')"
    [[ -n "$key" ]] || return 0
    mkdir -p "$(dirname "$saved")"
    (umask 077 && printf '%s\n' "$key" > "$saved")
    echo "Saved it to $saved." >&2
    printf '%s\n' "$key"
}

# The key for one of the servers other than Namespace.
build_server_key() {
    local server="$1" name="build-server-key.$1" file key
    local saved="${XDG_CONFIG_HOME:-$HOME/.config}/be3/$name"
    if [[ -n "${BE3_BUILD_SERVER_KEY:-}" ]]; then
        printf '%s\n' "$BE3_BUILD_SERVER_KEY"
        return 0
    fi
    for file in "$repository/.$name" "$saved"; do
        if [[ -f "$file" ]]; then
            tr -d '[:space:]' < "$file"
            echo ''
            return 0
        fi
    done
    key="$(ask_for_build_server_key "$server" 'key' "$saved")"
    if [[ -n "$key" ]]; then
        printf '%s\n' "$key"
        return 0
    fi
    echo "buck2 builds on $server, and there is no key for it. Set" >&2
    echo "BE3_BUILD_SERVER_KEY, or write the key to .$name at the root of the" >&2
    echo "checkout or to ~/.config/be3/$name. Run ./scripts/buck from a terminal" >&2
    echo 'to be asked for it. guides/build_server.md has more.' >&2
    return 1
}

# The host and the header for one of the servers other than Namespace, which
# serve the executor and the storage from the same host.
static_build_server_host() {
    case "$1" in
        buildbuddy) echo 'remote.buildbuddy.io' ;;
        *) echo "$1" ;;
    esac
}

static_build_server_header() {
    case "$1" in
        buildbuddy) echo "x-buildbuddy-api-key:$2" ;;
        *) echo "authorization:Bearer $2" ;;
    esac
}

# A value from the configuration nsc wrote: a host without its scheme or port,
# or the header as it is.
namespace_host() {
    sed -n "s|^$1 *= *\(grpcs*://\)\{0,1\}\([^:/]*\).*|\2|p" "$namespace_directory/buck2.buckconfig"
}

namespace_header() {
    sed -n 's/^http_headers *= *//p' "$namespace_directory/buck2.buckconfig"
}

# Whether the executor nsc last named answers a gRPC call. A cluster that has
# shut down answers 404 instead.
namespace_answers() {
    local engine header
    engine="$(namespace_host engine_address)"
    header="$(namespace_header)"
    [[ -n "$engine" && -n "$header" ]] || return 1
    (umask 077 && printf '%s\n' "${header/:/: }" > "$namespace_directory/probe-header")
    printf '\0\0\0\0\0' > "$namespace_directory/probe-body"
    curl --silent --max-time 20 --output /dev/null --dump-header - \
        --header 'content-type: application/grpc' --header 'te: trailers' \
        --header "@$namespace_directory/probe-header" \
        --data-binary "@$namespace_directory/probe-body" \
        "https://$engine/build.bazel.remote.execution.v2.Capabilities/GetCapabilities" 2> /dev/null |
        grep -qi '^content-type: *application/grpc'
}

# Asks nsc for the cluster's hosts when what target/namespace/ holds will not
# do, and says whether they changed.
refresh_namespace_cluster() {
    local nsc token stamp="$namespace_directory/asked" now age=999999 token_hash=''
    nsc="$(nsc_path)"
    if [[ ! -x "$nsc" ]]; then
        "$internal/install-nsc.sh" >&2
    fi
    mkdir -p "$namespace_directory"
    token="$(namespace_token_file)"
    if [[ -z "$token" ]]; then
        echo 'buck2 builds on Namespace, and there is no token for it. Set' >&2
        echo 'BE3_NAMESPACE_TOKEN, or write the token to .namespace-token.json at the root' >&2
        echo 'of the checkout or to ~/.config/be3/namespace-token.json, or run ./scripts/buck' >&2
        echo 'from a terminal to be asked for it. BE3_BUILD_SERVER picks another server.' >&2
        echo 'guides/build_server.md says how to make one.' >&2
        exit 1
    fi
    token_hash="$(sha256_of "$token")"
    now="$(date +%s)"
    if [[ -f "$stamp" && -f "$namespace_directory/buck2.buckconfig" ]] &&
        [[ "$(sed -n 2p "$stamp")" == "$token_hash" ]]; then
        age=$((now - $(sed -n 1p "$stamp")))
    fi
    if [[ $age -lt 600 ]] && ! $namespace_recheck; then
        return 0
    fi
    if [[ $age -lt 10800 ]] && namespace_answers; then
        return 0
    fi

    echo 'Waking the remote execution cluster with nsc...' >&2
    if ! (umask 077 && NS_DO_NOT_UPDATE=1 "$nsc" reapi setup buck2 \
        --token "$(native_path "$token")" --key "$namespace_cluster_key" \
        --config "$(native_path "$namespace_directory/buck2.buckconfig.partial")" \
        > "$namespace_directory/nsc.log" 2>&1); then
        cat "$namespace_directory/nsc.log" >&2
        echo '' >&2
        echo 'nsc could not set up the remote execution cluster (guides/build_server.md).' >&2
        exit 1
    fi
    mv -f "$namespace_directory/buck2.buckconfig.partial" "$namespace_directory/buck2.buckconfig"
    printf '%s\n%s\n' "$now" "$token_hash" > "$stamp"
    if [[ -z "$(namespace_host engine_address)" || -z "$(namespace_host cas_address)" || -z "$(namespace_header)" ]]; then
        echo "nsc's configuration, $namespace_directory/buck2.buckconfig, does not name the" >&2
        echo 'executor, the storage and the header buck2 needs.' >&2
        exit 1
    fi
}

configure_build_server() {
    local buck2="$1"
    local path="$repository/.buckconfig.local"
    local marker='# @generated by ./scripts/buck: where buck2 reaches the build server.'
    if [[ -f "$path" ]] && ! head -1 "$path" | grep -q '@generated by ./scripts/'; then
        echo '.buckconfig.local was written by hand, so buck2 is left to reach the build' >&2
        echo 'server it names.' >&2
        return 0
    fi
    local server engine storage header key proxy="${HTTPS_PROXY:-${https_proxy:-}}"
    server="$(build_server)" || exit 1
    if [[ "$server" == 'namespace' ]]; then
        refresh_namespace_cluster
        engine="$(namespace_host engine_address)"
        storage="$(namespace_host cas_address)"
        header="$(namespace_header)"
    else
        key="$(build_server_key "$server")" || exit 1
        engine="$(static_build_server_host "$server")"
        storage="$engine"
        header="$(static_build_server_header "$server" "$key")"
    fi
    local engine_address="$engine:443" storage_address="$storage:443" tls=true
    if [[ -n "$proxy" ]]; then
        assert_command go 'HTTPS_PROXY is set, and buck2 reaches the build server through it with scripts/internal/re-relay, which needs Go 1.24 or newer.'
        local version relay
        version="$(cat "$internal"/re-relay/* | cksum | cut -d ' ' -f 1)"
        relay="$repository/target/tools/re-relay-$version/re-relay$(go env GOEXE)"
        if [[ ! -x "$relay" ]]; then
            mkdir -p "$(dirname "$relay")"
            (cd "$internal/re-relay" && go build -trimpath -o "$relay.partial" .)
            mv -f "$relay.partial" "$relay"
        fi
        "$relay" ensure -listen "$re_relay_engine_address" -upstream "$engine" \
            -version "$version-$engine" -log "$repository/target/re-relay.log"
        "$relay" ensure -listen "$re_relay_storage_address" -upstream "$storage" \
            -version "$version-$storage" -log "$repository/target/re-relay.log"
        engine_address="$re_relay_engine_address"
        storage_address="$re_relay_storage_address"
        tls=false
    fi

    local config="$repository/target/buckconfig.local.partial"
    (
        umask 077
        {
            echo "$marker"
            echo "# The executor: $engine"
            echo "# The storage: $storage"
            echo '[buck2_re_client]'
            echo "action_cache_address = $storage_address"
            echo "cas_address = $storage_address"
            echo "engine_address = $engine_address"
            echo "http_headers = $header"
            echo "tls = $tls"
            echo '[be3]'
            echo "build_server = $server"
        } > "$config"
    )
    if cmp -s "$config" "$path"; then
        rm "$config"
    else
        mv -f "$config" "$path"
        "$buck2" kill > /dev/null 2>&1 || true
    fi
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

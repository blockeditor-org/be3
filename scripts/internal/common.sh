repository="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
internal="$repository/scripts/internal"
source "$internal/download.sh"

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

# The pinned releases: buck2, be, nsc and crates/bootstrap.
source "$internal/pins"

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

# Which build this checkout makes, for buck/app's build-info.json file: the hash of
# the working tree, taken through a throwaway index so untracked files count
# and ignored ones do not. It names the content rather than a commit, so a
# merge queue's commit, or any other commit of the same tree, is the same
# build, and everything that carries the id is cached across them.
build_id() {
    local index tree
    index="$(mktemp)"
    cp "$(git -C "$repository" rev-parse --path-format=absolute --git-path index 2> /dev/null)" "$index" 2> /dev/null || true
    tree="$(GIT_INDEX_FILE="$index" git -C "$repository" add -A 2> /dev/null &&
        GIT_INDEX_FILE="$index" git -C "$repository" write-tree 2> /dev/null)" || tree=''
    rm -f "$index"
    echo "${tree:-unknown}"
}

# Writes the build id where //buck/app:build-info.json reads it, buck/app/build-info.json,
# which git ignores. As a source file, a new id invalidates only what reads
# it, and it is rewritten only when it changes.
write_build_id() {
    local path="$repository/buck/app/build-info.json" id
    id="${BE3_BUILD_ID:-$(build_id)}"
    case "$id" in *[!a-zA-Z0-9._+-]*) echo "Invalid BE3_BUILD_ID" >&2; return 1 ;; esac
    local app_id="${BE3_APP_ID:-BlockDev}"
    case "$app_id" in Block|BlockDev) ;; *) echo "Invalid BE3_APP_ID" >&2; return 1 ;; esac
    if [[ "${BE3_DEPLOY_DEFAULT_SERVER:-false}" == true ]]; then
        id="$(printf '{"id":"%s","app_id":"%s","default_server":{"name":"Block","api_url":"https://api.be3.pfg.pw","auth_url":"https://api.be3.pfg.pw/auth","gateway_required":true}}' "$id" "$app_id")"
    else
        id="$(printf '{"id":"%s","app_id":"%s"}' "$id" "$app_id")"
    fi
    if [[ "$(cat "$path" 2> /dev/null)" != "$id" ]]; then
        printf '%s\n' "$id" > "$path.$$"
        mv -f "$path.$$" "$path"
    fi
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

# Where the pinned be is installed, under a directory named after the release
# so that moving the release installs the new one.
be_path() {
    echo "$repository/target/tools/be-$be_release/be"
}

write_if_changed() {
    if ! cmp -s "$1" "$2"; then
        cp "$1" "$2.partial"
        mv -f "$2.partial" "$2"
    fi
}

# Where buck2 reaches the build server, written into .buckconfig.local.
# BE3_BUILD_SERVER picks the server, or else the one a person at a terminal
# picked the first time and that is saved in ~/.config/be3/build-server:
# namespace, which is the default and CI's, or our own, api.be3.pfg.pw, or
# buildbuddy, or local, a NativeLink server a person runs themselves, reached at its host:port without TLS or a key. Each
# has its own key and its own cache. The file also names the server under
# [be3] build_server, since the workers' image is not the same on all of them
# (buck/tools/defs.bzl).
#
# Namespace's cluster shuts down when it has had nothing to do for a while, so
# its hosts are asked rather than kept anywhere, of namespace-waker at
# namespace_url, behind the API host's gateway, which runs
# `nsc reapi setup buck2` for every machine that builds: it starts the cluster if it is down and names its executor, its
# storage and the header that authenticates to them. That answer is kept in
# target/namespace/ and asked again when the executor stops answering, when it
# is three and a half hours old (an answer lasts about four), or when the
# gateway's header changes. The age is counted from when nsc wrote it, which the waker sends as
# an age header. The gateway's Be3-Gateway-Authorization header is
# BE3_AUTHORIZATION, as for ./scripts/be; in an agent's session the proxy adds
# it instead.
#
# The other servers' keys are BE3_BUILD_SERVER_KEY, or else
# .build-server-key.SERVER at the root of the checkout or
# ~/.config/be3/build-server-key.SERVER. With none of them, a person at a
# terminal is asked for it, and it is saved under ~/.config/be3 for every
# checkout; anything else - a pipe, CI, an agent's shell - is told what to set
# instead.
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
#
# Each checkout has relays of its own, on a pair of 2000 picked from its path,
# and shares them only when two paths pick the same pair. A relay serves one
# host, and a checkout that asks for another replaces it, which cut off every
# other checkout's calls through it, mid-download, and sent the rest to the
# wrong server.
re_relay_port="$((20000 + $(printf '%s' "$repository" | cksum | cut -d ' ' -f 1) % 2000 * 2))"
re_relay_engine_address="127.0.0.1:$re_relay_port"
re_relay_storage_address="127.0.0.1:$((re_relay_port + 1))"
namespace_directory="$repository/target/namespace"
namespace_url='https://api.be3.pfg.pw/build/namespace'

# Whether to look at the executor even when its hosts were asked for moments ago: after a
# command failed with an infrastructure error, the cluster may have gone.
namespace_recheck=false

# The servers BE3_BUILD_SERVER can name, in the order a person is offered them.
build_servers=(namespace api.be3.pfg.pw buildbuddy local)

# Where the server a person picked is saved: its name, and for local the
# address of its NativeLink on the next line.
saved_build_server_file() {
    echo "${XDG_CONFIG_HOME:-$HOME/.config}/be3/build-server"
}

# The server to build on: BE3_BUILD_SERVER, or else the one a person picked
# before. Without either, a person at a terminal is asked which, and anything
# else - a pipe, CI, an agent's shell - builds on Namespace.
build_server() {
    local server="${BE3_BUILD_SERVER:-}" saved
    saved="$(saved_build_server_file)"
    if [[ -z "$server" && -f "$saved" ]]; then
        server="$(sed -n 1p "$saved" | tr -d '[:space:]')"
    fi
    if [[ -z "$server" ]]; then
        server="$(ask_for_build_server "$saved")"
    fi
    server="${server:-namespace}"
    case "$server" in
        namespace | api.be3.pfg.pw | buildbuddy | local) echo "$server" ;;
        *)
            echo "The build server is $server, which is none of ${build_servers[*]}." >&2
            echo "BE3_BUILD_SERVER or $saved picks it." >&2
            return 1
            ;;
    esac
}

# Asks a person at a terminal which server to build on, and for local, where
# its NativeLink listens, and saves the answer. Without a terminal it prints
# nothing.
ask_for_build_server() {
    local saved="$1" choice server='' address=''
    [[ -t 0 && -t 2 ]] || return 0
    echo 'Which build server should buck2 build on? guides/build_server.md says more.' >&2
    local index=1
    for server in "${build_servers[@]}"; do
        case "$server" in
            namespace) echo "  $index) namespace: Namespace's remote execution" >&2 ;;
            api.be3.pfg.pw) echo "  $index) api.be3.pfg.pw: our own NativeLink server" >&2 ;;
            buildbuddy) echo "  $index) buildbuddy: BuildBuddy's remote.buildbuddy.io" >&2 ;;
            local) echo "  $index) local: a NativeLink server of your own, without TLS or a key" >&2 ;;
        esac
        index=$((index + 1))
    done
    server=''
    while [[ -z "$server" ]]; do
        read -r -p "Build server [1-${#build_servers[@]}]: " choice || return 0
        choice="$(printf '%s' "$choice" | tr -d '[:space:]')"
        if [[ "$choice" =~ ^[0-9]+$ && "$choice" -ge 1 && "$choice" -le ${#build_servers[@]} ]]; then
            server="${build_servers[$((choice - 1))]}"
        fi
    done
    if [[ "$server" == 'local' ]]; then
        address="$(ask_for_local_build_server_address)"
        [[ -n "$address" ]] || return 0
    fi
    mkdir -p "$(dirname "$saved")"
    printf '%s\n' "$server" > "$saved"
    if [[ -n "$address" ]]; then
        printf '%s\n' "$address" >> "$saved"
    fi
    echo "Saved it to $saved; delete it to be asked again." >&2
    printf '%s\n' "$server"
}

# A NativeLink address as buck2 takes it: a bare host:port.
local_build_server_address_is_valid() {
    [[ "$1" =~ ^[^/:[:space:]]+:[0-9]+$ ]]
}

ask_for_local_build_server_address() {
    local address=''
    while true; do
        read -r -p 'NativeLink address (such as 127.0.0.1:50052): ' address || return 0
        address="$(printf '%s' "$address" | tr -d '[:space:]')"
        address="${address#grpc://}"
        if local_build_server_address_is_valid "$address"; then
            printf '%s\n' "$address"
            return 0
        fi
        echo 'That is not a host:port.' >&2
    done
}

# Where the local server listens: BE3_BUILD_SERVER_ADDRESS, or else the
# address saved with the choice, or else, at a terminal, what a person types.
local_build_server_address() {
    local address="${BE3_BUILD_SERVER_ADDRESS:-}" saved
    saved="$(saved_build_server_file)"
    if [[ -z "$address" && -f "$saved" && "$(sed -n 1p "$saved" | tr -d '[:space:]')" == 'local' ]]; then
        address="$(sed -n 2p "$saved" | tr -d '[:space:]')"
    fi
    if [[ -z "$address" && -t 0 && -t 2 ]]; then
        address="$(ask_for_local_build_server_address)"
        if [[ -n "$address" ]]; then
            mkdir -p "$(dirname "$saved")"
            printf 'local\n%s\n' "$address" > "$saved"
        fi
    fi
    if [[ -z "$address" ]]; then
        echo 'buck2 builds on a local NativeLink server, and nothing says where it is. Set' >&2
        echo "BE3_BUILD_SERVER_ADDRESS to its host:port, such as 127.0.0.1:50052." >&2
        return 1
    fi
    if ! local_build_server_address_is_valid "$address"; then
        echo "The local build server's address is $address, which is not a host:port." >&2
        return 1
    fi
    printf '%s\n' "$address"
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

# The key for one of the servers: Namespace's is the one for its waker at
# namespace_url, and has a variable of its own, so that an environment can hold
# it beside another server's.
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

# Asks for the cluster's hosts when what target/namespace/ holds will not do,
# writing them to target/namespace/buck2.buckconfig.
refresh_namespace_cluster() {
    local stamp="$namespace_directory/asked" now age=999999 key_hash status answer_age
    local header="$namespace_directory/request-header" answer="$namespace_directory/buck2.buckconfig.partial"
    mkdir -p "$namespace_directory"
    : > "$header"
    if [[ -n "${BE3_AUTHORIZATION:-}" ]]; then
        (umask 077 && printf 'Be3-Gateway-Authorization: %s\n' "$BE3_AUTHORIZATION" > "$header")
    fi
    key_hash="$(sha256_of "$header")"
    now="$(date +%s)"
    if [[ -f "$stamp" && -f "$namespace_directory/buck2.buckconfig" ]] &&
        [[ "$(sed -n 2p "$stamp")" == "$key_hash" ]]; then
        age=$((now - $(sed -n 1p "$stamp")))
    fi
    if [[ $age -lt 600 ]] && ! $namespace_recheck; then
        return 0
    fi
    if [[ $age -lt 12600 ]] && namespace_answers; then
        return 0
    fi

    echo "Waking the remote execution cluster through $namespace_url..." >&2
    if ! status="$(curl --silent --show-error --max-time 600 --header "@$header" \
        --dump-header "$namespace_directory/response-header" \
        --output "$answer" --write-out '%{http_code}' "$namespace_url")"; then
        echo "Could not reach $namespace_url (guides/build_server.md)." >&2
        exit 1
    fi
    if [[ "$status" != 200 ]]; then
        cat "$answer" >&2 2> /dev/null || true
        echo '' >&2
        if [[ "$status" == 401 || "$status" == 403 ]]; then
            echo "The gateway in front of $namespace_url refused this request. Set" >&2
            echo 'BE3_AUTHORIZATION to its Be3-Gateway-Authorization header (guides/build_server.md).' >&2
        else
            echo "$namespace_url answered $status rather than the cluster's hosts (guides/build_server.md)." >&2
        fi
        exit 1
    fi
    mv -f "$answer" "$namespace_directory/buck2.buckconfig"
    answer_age="$(sed -n 's/^[Aa]ge: *\([0-9]*\).*/\1/p' "$namespace_directory/response-header")"
    printf '%s\n%s\n' "$((now - ${answer_age:-0}))" "$key_hash" > "$stamp"
    if [[ -z "$(namespace_host engine_address)" || -z "$(namespace_host cas_address)" || -z "$(namespace_header)" ]]; then
        echo "$namespace_url answered with a configuration that does not name the executor," >&2
        echo "the storage and the header buck2 needs: $namespace_directory/buck2.buckconfig." >&2
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
    local engine_address storage_address tls=true
    if [[ "$server" == 'local' ]]; then
        engine="$(local_build_server_address)" || exit 1
        storage="$engine"
        engine_address="$engine"
        storage_address="$engine"
        header=''
        tls=false
        proxy=''
    elif [[ "$server" == 'namespace' ]]; then
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
    if [[ "$server" != 'local' ]]; then
        engine_address="$engine:443"
        storage_address="$storage:443"
    fi
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
            if [[ -n "$header" ]]; then
                echo "http_headers = $header"
            fi
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


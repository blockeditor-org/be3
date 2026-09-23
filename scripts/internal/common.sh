repository="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
internal="$repository/scripts/internal"

# How long everything took.
#
# A build is a dozen steps of wildly different cost, and nothing in the output
# says which one a slow build was spent in. So each step announces itself
# through step, what it cost is reported when the next step begins or when the
# script ends, and every build and run script reports its own total.
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

# A step no clock of the script's can hold, because it runs in the background
# beside another one. It reports itself when it finishes rather than when the
# next step starts, and it reports even when the command failed, which is the
# run worth knowing the cost of.
run_step() {
    local name="$1" started status
    shift
    started="$(now_ms)"
    echo "$name..."
    if "$@"; then
        status=0
    else
        status=$?
    fi
    echo "  $name took $(format_duration "$(($(now_ms) - started))")"
    return "$status"
}

script_started=''
script_name=''

# The total every build and run script ends with. It is an exit trap rather
# than a last line so that a script killed by a failing step still says how
# long it spent getting there, and so the step that failed is closed rather
# than left hanging. The name is what one script runs another for: a run
# reports its own total after the build it delegated reported that one, and
# only the name says which is which.
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

# clang and rust-lld are native programs even when the build runs under a POSIX
# shell on Windows, and neither can open the /c/... form such a shell hands out,
# so every path that travels to one of them as an argument goes through here.
native_path() {
    if [[ "${OS:-}" == 'Windows_NT' ]] && command -v cygpath > /dev/null; then
        cygpath -m "$1"
    else
        echo "$1"
    fi
}

manifest_field() {
    sed -n "s/.*\"$2\"[[:space:]]*:[[:space:]]*\"\([^\"]*\)\".*/\1/p" "$1" | head -1
}

# What a full build is, and what a default one leaves out.
#
# Three things in the workspace cost far more than they contribute to a check of
# everything else. libghostty-vt has to be compiled out of a Ghostty checkout by
# a Zig toolchain before cargo can link the app at all, which is half a gigabyte
# of downloads and the one prerequisite a restricted network cannot fetch; the
# embedded browser is fifty-nine crates that nothing else in the workspace
# needs; and cvl2 has nothing depending on it at all.
#
# So a default build leaves all three out, which is what makes a fresh checkout
# compile with nothing but cargo, and a full build puts them back. ./scripts/check
# and ./scripts/verify take --full, every script reads BE3_FULL, and CI sets it,
# so what ships and what is linted is still the whole app.
full_build() {
    [[ -n "${BE3_FULL:-}" ]]
}

# Packages only a full build compiles. ghostty-vt is not merely unused without
# one: its build script has no archive to point cargo at, so it fails to build.
optional_packages=(ghostty-vt cvl2)

# The packages a workspace-wide cargo call in these scripts is for, in the
# variable `selection`. Feature selection travels with it, because a call that
# named the packages one way and the features another would resolve features
# over a different graph and compile everything a second time.
workspace_selection() {
    selection=(--workspace)
    local package
    if full_build; then
        selection+=(--features block-app/full)
    else
        for package in "${optional_packages[@]}"; do
            selection+=(--exclude "$package")
        done
    fi
}

# The same selection without the plugins, whose tests are compiled to wasm and
# run by internal/test-plugins.sh rather than by the native run.
native_selection() {
    load_plugins
    workspace_selection
    local plugin
    for plugin in "${plugins[@]}"; do
        selection+=(--exclude "$plugin")
    done
}

# Builds libghostty-vt, and only when there is a build to link it into. Every
# script that compiles the app calls this rather than the script itself, so the
# Zig toolchain and the Ghostty checkout are a cost a full build pays and
# nothing else even has to have.
ensure_ghostty_vt() {
    if ! full_build; then
        return 0
    fi
    step "Building libghostty-vt for $1"
    "$internal/build-ghostty-vt.sh" --triple "$1" > /dev/null
    end_step
}

load_plugins() {
    plugins=()
    local manifest
    for manifest in "$repository"/crates/editors/*/manifest.json; do
        [[ -f "$manifest" ]] || continue
        plugins+=("$(basename "$(dirname "$manifest")")")
    done
    if [[ ${#plugins[@]} -eq 0 ]]; then
        echo 'No plugin manifests were found under crates/editors' >&2
        exit 1
    fi
}

# Packages that are not plugins themselves but whose tests only exist on wasm,
# so a native run never builds them and a break in them is invisible until
# someone compiles a plugin. block-editor-plugin's editor session is the whole
# guest half of the plugin framework and is behind `cfg(target_arch =
# "wasm32")`, so its tests belong to the plugin run rather than the native one.
guest_only_packages=(block-editor-plugin)

# Everything internal/test-plugins.sh compiles to wasm and runs through the
# plugin host: every plugin, and the guest-only packages beside them.
load_wasm_tested() {
    load_plugins
    wasm_tested=("${plugins[@]}" "${guest_only_packages[@]}")
}

plugin_manifest() {
    echo "$repository/crates/editors/$1/manifest.json"
}

plugin_id() {
    local manifest id
    manifest="$(plugin_manifest "$1")"
    if [[ ! -f "$manifest" ]]; then
        echo "No manifest at $manifest" >&2
        return 1
    fi
    id="$(manifest_field "$manifest" id)"
    if [[ -z "$id" ]]; then
        echo "$manifest has no plugin id" >&2
        return 1
    fi
    echo "$id"
}

# Puts every plugin's manifest beside the artifacts cargo produced, named after
# the plugin's id, and leaves the names it wrote in plugin_manifests. Nothing a
# plugin was compiled to is moved: the manifest names its entry point and the
# app resolves that against the directory the manifest itself was found in.
stage_plugin_manifests() {
    local directory="$1"
    mkdir -p "$directory"
    rm -f "$directory"/*.plugin.json
    plugin_manifests=()
    local plugin id
    for plugin in "${plugins[@]}"; do
        id="$(plugin_id "$plugin")"
        cp "$(plugin_manifest "$plugin")" "$directory/$id.plugin.json"
        plugin_manifests+=("$id.plugin.json")
    done
}

# The browser and Android read an index because neither can list a directory.
# Native discovery scans for the manifests instead, so it needs no index.
write_plugin_index() {
    write_index "$1/plugins.json" "${plugin_manifests[@]}"
}

write_index() {
    local file="$1"
    shift
    local entries=("$@")
    local index separator
    {
        echo '['
        for index in "${!entries[@]}"; do
            separator=','
            if [[ $index -eq $((${#entries[@]} - 1)) ]]; then
                separator=''
            fi
            echo "  \"${entries[$index]}\"$separator"
        done
        echo ']'
    } > "$file"
}

load_games() {
    games=()
    local manifest
    for manifest in "$repository"/crates/tabletop_games/rules/*/Cargo.toml; do
        [[ -f "$manifest" ]] || continue
        games+=("$(basename "$(dirname "$manifest")")")
    done
    if [[ ${#games[@]} -eq 0 ]]; then
        echo 'No games were found under crates/tabletop_games/rules' >&2
        exit 1
    fi
}

games_directory=''

# Compiles every game to its own WebAssembly module in one cargo call and
# leaves them where cargo put them, in games_directory. Nothing ships them: a
# module reaches the app as a game module block the user imports the file into,
# so the build only has to produce a file there is something to import.
build_games() {
    local profile="$1"
    local arguments=(--lib --target wasm32-unknown-unknown)
    if [[ "$profile" == 'release' ]]; then
        arguments+=(--release)
    fi
    if ! rustup target list --installed | grep -qx 'wasm32-unknown-unknown'; then
        echo 'Installing the wasm32-unknown-unknown Rust target...'
        rustup target add wasm32-unknown-unknown
    fi

    load_games
    local game
    for game in "${games[@]}"; do
        arguments+=(-p "$game")
    done
    step "Building ${#games[@]} games"
    (
        cd "$repository"
        unwrap_rustc_for_wasm
        cargo build "${arguments[@]}"
    )
    end_step

    games_directory="$repository/target/wasm32-unknown-unknown/$profile"
    for game in "${games[@]}"; do
        if [[ ! -f "$games_directory/$game.wasm" ]]; then
            echo "cargo did not produce $games_directory/$game.wasm" >&2
            exit 1
        fi
    done
}

# Both the WASI toolchain and the guest flags a plugin needs to compile without
# wasm-bindgen. The web build exports the same environment for its own cargo
# call; a plugin built for wasmtime is the same target with none of the glue.
wasi_sdk_version='33'
# The sysroot every plugin and the web bundle are linked against, so of the
# archives fetched here this is the one that reaches what CI publishes: the web
# and plugin jobs both download it on a cache miss.
wasi_sysroot_sha256='063bc1b56582b9923e08ac9b89e58789618d851763f01530b3ff20b9e5df0ca3'
wasm_rust_target='wasm32-wasip1-threads'

# The flags below are LLVM 19's, so an older clang stops at the first of them
# with nothing built. Ubuntu still ships 18 as plain clang while packaging newer
# ones beside it, so a machine that has one is used rather than turned away.
wasm_clang_minimum='19'
wasm_clang=''
wasm_clangxx=''
wasm_ar=''

clang_major() {
    command -v "$1" > /dev/null || return 1
    "$1" --version 2> /dev/null | sed -n 's/.*clang version \([0-9][0-9]*\).*/\1/p' | head -1
}

# The newest clang on PATH that is new enough, or nothing. ./scripts/setup asks
# so that it only installs a clang when the machine has none it could use, and
# pick_wasm_clang below turns the same answer into the three tools the wasm
# build runs.
newest_wasm_clang() {
    local candidate major
    for candidate in clang clang-22 clang-21 clang-20 clang-19; do
        major="$(clang_major "$candidate" || true)"
        [[ -n "$major" ]] || continue
        if [[ "$major" -ge "$wasm_clang_minimum" ]]; then
            echo "$candidate"
            return 0
        fi
    done
    return 1
}

# Whether a machine already has everything the wasm build compiles HarfBuzz
# with, which is a clang new enough and the llvm-ar that goes with it. Only
# ./scripts/setup asks, to decide whether there is a clang to install.
wasm_clang_installed() {
    local candidate
    candidate="$(newest_wasm_clang)" || return 1
    command -v "${candidate/clang/llvm-ar}" > /dev/null \
        || command -v llvm-ar > /dev/null
}

pick_wasm_clang() {
    local newest
    newest="$(newest_wasm_clang || true)"
    if [[ -z "$newest" ]]; then
        echo "The wasm build needs clang $wasm_clang_minimum or newer, and none was found on PATH." >&2
        echo "Install one (apt-get install clang-20 llvm-20) and run this again." >&2
        exit 1
    fi
    wasm_clang="$newest"
    wasm_clangxx="${newest/clang/clang++}"
    command -v "$wasm_clangxx" > /dev/null || wasm_clangxx='clang++'
    wasm_ar="${newest/clang/llvm-ar}"
    command -v "$wasm_ar" > /dev/null || wasm_ar='llvm-ar'
    assert_command "$wasm_ar" 'Install LLVM and put its bin directory on PATH.'
}

# An extraction that was interrupted leaves the headers behind without the
# archives the link needs, so what is checked for is what is actually linked
# against rather than the directory merely existing.
wasi_sysroot_is_complete() {
    [[ -d "$1/include" \
        && -d "$1/lib/$wasm_rust_target/noeh" \
        && -f "$1/lib/$wasm_rust_target/libsetjmp.a" ]]
}

export_wasi_toolchain() {
    local requested="${1:-}"
    unwrap_rustc_for_wasm
    pick_wasm_clang
    if ! rustup target list --installed | grep -qx "$wasm_rust_target"; then
        echo "Installing the $wasm_rust_target Rust target..."
        rustup target add "$wasm_rust_target"
    fi
    wasi_sysroot="$requested"
    if [[ -z "$wasi_sysroot" ]]; then
        local tools="$repository/target/tools"
        wasi_sysroot="$tools/wasi-sysroot"
        # The hash of the archive it came out of, so that a sysroot left by a
        # different pin is replaced rather than linked against. CI restores this
        # directory from a cache keyed on the same hash (see
        # .github/actions/wasi-sysroot), so a bumped wasi_sdk_version misses
        # that cache instead of quietly building against the old sysroot.
        local stamp="$wasi_sysroot/.stamp"
        if ! wasi_sysroot_is_complete "$wasi_sysroot" \
            || [[ "$(cat "$stamp" 2> /dev/null)" != "$wasi_sysroot_sha256" ]]; then
            local archive="$tools/wasi-sysroot.tar.gz"
            local extracted="$tools/wasi-sysroot-$wasi_sdk_version.0+m"
            local url="https://github.com/WebAssembly/wasi-sdk/releases/download/wasi-sdk-$wasi_sdk_version/wasi-sysroot-$wasi_sdk_version.0+m.tar.gz"
            # This is the download that has 504'd on us, and the one every
            # published plugin and the web bundle are linked against, so it is
            # the first the mirror above carries.
            # Named exactly as upstream names it, "+m" and all, so that the
            # hash pinned above is a hash of the same bytes either way rather
            # than of a repackaging.
            local mirror="$download_mirror/wasi-sysroot-$wasi_sdk_version.0+m.tar.gz"
            step "Downloading the WASI sysroot from $url"
            mkdir -p "$tools"
            rm -rf "$wasi_sysroot" "$extracted"
            download_verified "$url" "$archive" "$wasi_sysroot_sha256" "$mirror"
            tar -xzf "$archive" -C "$tools"
            mv "$extracted" "$wasi_sysroot"
            rm "$archive"
            # Written last, so an interrupted extraction leaves a directory
            # that fails the check above rather than one that passes it.
            echo "$wasi_sysroot_sha256" > "$stamp"
            end_step
        fi
    fi
    if ! wasi_sysroot_is_complete "$wasi_sysroot"; then
        echo "The WASI sysroot at $wasi_sysroot has no lib/$wasm_rust_target to link against." >&2
        echo 'Delete it and run the build again to fetch it afresh, or pass --wasi-sysroot a complete one.' >&2
        exit 1
    fi
    wasi_sysroot="$(cd "$wasi_sysroot" && pwd)"
    local sysroot
    sysroot="$(native_path "$wasi_sysroot")"
    # cc and rustc both split these variables on whitespace, so a sysroot with a
    # space in its path arrives at clang as two flags that mean nothing.
    if [[ "$sysroot" == *' '* ]]; then
        echo "The WASI sysroot path contains a space, which cc and rustc split on: $sysroot" >&2
        echo 'Pass --wasi-sysroot a path without spaces, or move the checkout to one.' >&2
        exit 1
    fi
    echo "Building against the WASI sysroot at $sysroot"
    # -pthread is what marks the C objects as using atomics and bulk memory,
    # which is what lets them be linked into a module with a shared memory at
    # all. HarfBuzz itself is still left single-threaded, because only the
    # thread that draws shapes text and HB_NO_MT keeps it from paying for locks
    # nothing contends.
    local flags="--sysroot=$sysroot -isystem $sysroot/include/c++/v1 -pthread -mllvm -wasm-enable-sjlj -mllvm -wasm-use-legacy-eh=false -fno-exceptions -fno-rtti -DHB_NO_MT -O2 -w"
    export CC_wasm32_wasip1_threads="$wasm_clang"
    export CXX_wasm32_wasip1_threads="$wasm_clangxx"
    export AR_wasm32_wasip1_threads="$wasm_ar"
    export CFLAGS_wasm32_wasip1_threads="$flags"
    export CXXFLAGS_wasm32_wasip1_threads="$flags"
    export CXXSTDLIB_wasm32_wasip1_threads='c++'
    export HARFBUZZ_SYS_NO_PKG_CONFIG='1'
    # rustc links a cdylib with --no-entry, so lld strips the symbols a host
    # needs to lay out a thread's own storage. Exporting them is what lets it
    # turn the module into one that can be instantiated more than once.
    # --no-entry also leaves the module with nothing that runs libc's
    # constructors, which a command module reaches through _start. A guest
    # whose constructors never ran has a libc that believes it has no thread
    # storage, and the first thread it is asked to start deadlocks it, so the
    # host calls __wasm_call_ctors itself and the plugin has to export it.
    local exports=''
    local symbol
    for symbol in __heap_base __tls_base __tls_size __tls_align __wasm_init_tls __wasm_call_ctors; do
        exports+=" -C link-arg=--export=$symbol"
    done
    export RUSTFLAGS="-C link-arg=-L$sysroot/lib/$wasm_rust_target/noeh -C link-arg=$sysroot/lib/$wasm_rust_target/libsetjmp.a$exports"
}

# A plugin is its own cargo call: the app links wgpu with real backends and a
# guest links it with only the custom one, and a single call would unify the two
# into a guest that carries a backend it cannot use. It also gets its own
# profile, because an unoptimised guest is hundreds of megabytes of wasm that
# Cranelift then spends minutes compiling at every launch.
build_plugin_wasm() {
    local profile="$1" destination="$2"
    local wasm_profile='plugin'
    if [[ "$profile" == 'release' ]]; then
        wasm_profile='plugin-release'
    fi
    local arguments=(--target "$wasm_rust_target" --profile "$wasm_profile")
    local plugin selection=()
    for plugin in "${plugins[@]}"; do
        selection+=(-p "$plugin")
    done
    step "Building ${#plugins[@]} plugins for $wasm_rust_target"
    (
        export_wasi_toolchain "${wasi_sysroot:-}"
        cargo build "${arguments[@]}" "${selection[@]}"
    )
    local built="$repository/target/$wasm_rust_target/$wasm_profile"
    mkdir -p "$destination"
    for plugin in "${plugins[@]}"; do
        local module="$built/${plugin//-/_}.wasm"
        if [[ ! -f "$module" ]]; then
            echo "cargo did not produce $module" >&2
            exit 1
        fi
        cp -p "$module" "$destination/$plugin.wasm"
    done
    end_step
}

# Cranelift compiles a plugin the first time the app opens it, which is seconds
# of work for a module this size and happens again on every machine the build
# lands on. Compiling it here instead leaves a .cwasm beside the .wasm that
# wasmtime maps straight in. The app still reads the .wasm whenever an artifact
# is missing or was made by a different wasmtime, so a plain cargo build stays
# usable; a stale artifact is one the app would fall back from, so the work is
# redone whenever the module or the compiler that produced it is newer.
precompiler=''

# Names the compiler a cargo call has already produced in the directory it
# builds into, so a build that compiled it alongside the app does not compile it
# a second time here.
use_precompiler() {
    local built="$1/precompile"
    if [[ -f "$built.exe" ]]; then
        built+='.exe'
    fi
    if [[ ! -f "$built" ]]; then
        echo "cargo did not produce $built" >&2
        exit 1
    fi
    precompiler="$built"
}

# The compiler runs on the machine the build runs on whatever the app is being
# built for, so only a build for this machine can hand it over from its own
# cargo call. A cross build compiles it here instead, where all-arch is what
# gives wasmtime the backend the app's architecture needs.
build_precompiler() {
    if [[ -n "$precompiler" ]]; then
        return
    fi
    step 'Building the plugin compiler'
    (cd "$repository" && cargo build --release -p block-wasm-host --features all-arch --bin precompile)
    use_precompiler "$repository/target/release"
    end_step
}

precompile_plugin_wasm() {
    local directory="$1" triple="$2"
    build_precompiler
    local plugin module artifact stale=()
    for plugin in "${plugins[@]}"; do
        module="$directory/$plugin.wasm"
        artifact="$directory/$plugin.cwasm"
        if [[ -f "$artifact" && "$artifact" -nt "$module" && "$artifact" -nt "$precompiler" ]]; then
            continue
        fi
        stale+=("$module")
    done
    if [[ ${#stale[@]} -eq 0 ]]; then
        echo "Every plugin is already compiled for $triple"
        return
    fi
    step "Compiling ${#stale[@]} plugins for $triple"
    "$precompiler" --target "$triple" "${stale[@]}"
    end_step
}

# ./scripts/verify runs the tests through nextest, which is not a rustup
# component. internal/install-nextest.sh fetches this release rather than
# building it, because `cargo install cargo-nextest` spends around four minutes
# compiling a test runner nothing in the workspace depends on, which was most of
# what setting a machine up cost. The musl builds run on any glibc a machine
# happens to have, which the gnu ones do not.
nextest_version='0.9.145'
nextest_sha256_x86_64='cd3c85194e8b28ad26676d287f41f0d6b4d456cef5bd94700def5dea8e328883'
nextest_sha256_aarch64='103a8f68b20fb01ee5daf2eb4c037b54ace550a04a288ee35761c5e7b0351311'

# sccache stands between cargo and rustc and answers a compilation from a shared
# object store whenever some other machine has already compiled that crate with
# those flags, which is most of what a cold checkout or a CI runner spends its
# first build on. Every profile in Cargo.toml already turns incremental
# compilation off, so nothing is given up by routing rustc through it.
#
# The store is a Bunny storage zone. Its read-only key is in the clear on
# purpose: a fresh clone and a pull request from a fork can both read what the
# project has already built without holding a secret. Writing needs the key that
# is not public, which arrives as BUNNY_SCCACHE_PASSWORD, from a repository
# secret in CI and from the environment of whoever has it locally. Without it
# the cache is read-only, which costs a miss nothing but a locally reported
# write error.
sccache_version='0.18.0'
# internal/install-sccache.sh fetches the musl release for one of these two
# architectures and checks it against the hash here. It becomes RUSTC_WRAPPER,
# so it wraps every rustc and, off Windows, every cc the build runs: of
# everything downloaded here this is the one with the most to see.
sccache_sha256_x86_64='45f1447fbe231e3037bde351ef70677dd212216c8d62ae7ca409fecc4d6acc89'
sccache_sha256_aarch64='2b3284d5da3b46a47dc4229e75bb7b88ac4aa99c8d754fb7d2f84997e5a4354a'
sccache_directory="$repository/target/tools/sccache"
sccache_bucket='sccache'
sccache_read_only_password='11737dc0-6417-4dcc-a076dc81ac00-71a7-461f'

# internal/install-sccache.sh puts one under target, and CI installs one on
# PATH; either will do. Cargo spawns the wrapper itself rather than through a
# shell, so the one on PATH is left as a bare name for cargo to resolve, and
# only the installed copy travels as a path, which on Windows has to be a form
# a native program can open.
find_sccache() {
    if [[ -x "$sccache_directory/sccache" ]]; then
        native_path "$sccache_directory/sccache"
    elif command -v sccache > /dev/null; then
        echo 'sccache'
    fi
}

# Sourcing this file is what turns the cache on, so every script that builds
# gets it without having to ask. A machine with no sccache installed, one whose
# cargo is already pointed at a wrapper, and one that set BE3_NO_SCCACHE are all
# left exactly as they were.
configure_sccache() {
    if [[ -n "${RUSTC_WRAPPER:-}" || -n "${BE3_NO_SCCACHE:-}" ]]; then
        return
    fi
    local binary
    binary="$(find_sccache)"
    if [[ -z "$binary" ]]; then
        return
    fi

    export RUSTC_WRAPPER="$binary"
    # The -sys crates compile their C with cc-rs rather than with rustc, and
    # RUSTC_WRAPPER never sees any of it: HarfBuzz, zstd and the bundled SQLite
    # alone are a minute of every cold build, repeated on every machine. sccache
    # caches a C compile the same way it caches a Rust one, so point cc-rs at it
    # too. Only the host compilers are wrapped; the wasm ones are set by
    # export_wasi_toolchain, which needs the real clang for its own flags.
    #
    # Everywhere but Windows, cc-rs would have run the cc and c++ on PATH
    # anyway, so naming them here changes nothing but who spawns them. On a
    # Windows target it would change the compiler: cc-rs pays no attention to
    # PATH there and asks the registry for the MSVC cl.exe cargo links objects
    # with, while the cc a POSIX shell finds is the mingw gcc the runner image
    # happens to ship. Setting CC hands zstd and SQLite to that gcc, and
    # link.exe then has objects wanting libgcc's ___chkstk_ms and a real
    # fprintf, which the UCRT only has as an inline. So the C half of the cache
    # is for the platforms whose compiler is the one on PATH; the Rust half,
    # which is nearly all of a Windows job, is wrapped there as everywhere.
    if [[ "${OS:-}" != 'Windows_NT' ]]; then
        if [[ -z "${CC:-}" ]] && command -v cc > /dev/null; then
            export CC="$binary cc"
        fi
        if [[ -z "${CXX:-}" ]] && command -v c++ > /dev/null; then
            export CXX="$binary c++"
        fi
    fi
    export SCCACHE_BUCKET="$sccache_bucket"
    export SCCACHE_ENDPOINT='https://de-s3.storage.bunnycdn.com'
    # Bunny serves one region per endpoint and pays no attention to this, but
    # the S3 signature it does check is computed over the region name, so both
    # ends have to spell it the same way.
    export SCCACHE_REGION='de'
    # Bunny's S3 gateway takes the storage zone as the access key and a zone
    # password as the secret.
    export AWS_ACCESS_KEY_ID="$sccache_bucket"
    if [[ -n "${BUNNY_SCCACHE_PASSWORD:-}" ]]; then
        export AWS_SECRET_ACCESS_KEY="$BUNNY_SCCACHE_PASSWORD"
        export SCCACHE_S3_RW_MODE='READ_WRITE'
    else
        export AWS_SECRET_ACCESS_KEY="$sccache_read_only_password"
        # Left out, sccache would upload every miss and take a 403 for it.
        export SCCACHE_S3_RW_MODE='READ_ONLY'
    fi

    # Before the server will answer its first client it proves the store
    # reachable, by reading and writing a probe object, and the client that
    # spawned it waits ten seconds for that and no longer. So a slow answer from
    # Bunny does not cost a cache hit, it fails the compile that was waiting on
    # the server and the build with it, which is how a CI job came to die on
    # `sccache rustc -vV` with nothing built. The probe reports only the access
    # SCCACHE_S3_RW_MODE above has already settled, so skipping it gives up
    # nothing, takes the network off the startup path entirely, and leaves a
    # store that cannot be reached costing what it ought to: a miss.
    export SCCACHE_SKIP_CACHE_CHECK='1'

    mkdir -p "$repository/target"

    # With the probe gone, starting a server is spawning a process and binding a
    # socket, but ten seconds is still a tight cap on a loaded machine for
    # something that fails the whole build rather than one compilation. Only a
    # config file can raise it, so write one; nothing else belongs in it, and a
    # machine already pointing SCCACHE_CONF at a config of its own is left alone.
    if [[ -z "${SCCACHE_CONF:-}" ]]; then
        local configuration="$repository/target/sccache-config.toml"
        local startup='server_startup_timeout_ms = 60000'
        if [[ "$(cat "$configuration" 2> /dev/null)" != "$startup" ]]; then
            echo "$startup" > "$configuration"
        fi
        SCCACHE_CONF="$(native_path "$configuration")"
        export SCCACHE_CONF
    fi

    # The server is started by whichever compilation first wanted it and holds
    # no terminal of its own, so with nowhere to write, everything it has to say
    # about a refused key or a store that stopped answering is lost and the
    # build only quietly reports misses. Point its stderr at a file instead.
    #
    # The filter is env_logger's, and it is a filter rather than a level because
    # the store is reached through opendal, which reports a miss as a failed
    # read at warning level: left in, a cold build writes one of those for every
    # crate it compiles and buries the four lines actually worth reading. What
    # opendal calls a failure rather than an error, which is the 403 of a key
    # that stopped being accepted, is logged above that and survives the filter.
    # `SCCACHE_LOG=debug` in the environment overrides all of this and asks the
    # same file for every request the server served.
    #
    # The file has to be openable before the server daemonises, and one that
    # cannot open it exits without ever reporting itself, which the client can
    # only read as the timeout above.
    SCCACHE_ERROR_LOG="$(native_path "$repository/target/sccache.log")"
    export SCCACHE_ERROR_LOG
    export SCCACHE_LOG="${SCCACHE_LOG:-sccache=info,opendal=error}"
}

configure_sccache

# What a Windows job cannot cache, and why it is only the wasm half of one.
#
# Cargo lists every feature a crate declares in a single --check-cfg when it
# compiles that crate, and web-sys declares four thousand of them: one argument
# past the 32k a Windows command line holds. Cargo stays under the cap by
# handing rustc a response file, but sccache reads that file and spawns rustc
# itself with every argument written out, so the compile dies with "The
# filename or extension is too long".
#
# Nothing on this side can shorten that argument, so a wasm cargo call on
# Windows goes straight to rustc. The native build keeps the cache, which is
# where a Windows job spends most of its time and where nothing compiled comes
# near the cap.
unwrap_rustc_for_wasm() {
    if [[ "${OS:-}" == 'Windows_NT' ]]; then
        unset RUSTC_WRAPPER
    fi
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
# from source. Nothing but ./scripts/buckify needs it: the file it writes is
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

# The Starlark formatter from the same release, which ./scripts/verify runs over
# the BUCK files the way it runs rustfmt over the Rust.
starlark_fmt_sha256_x86_64_unknown_linux_gnu='7235fa17c2d937da5c509a7576049239a1aca854369863075cebf9ff181033ca'
starlark_fmt_sha256_aarch64_unknown_linux_gnu='4b8c37092d8586c145b4429ad2b3f8a671c78084dc29186f7c2ed6827d8ef170'
starlark_fmt_sha256_x86_64_apple_darwin='3f891528bf55f25b69b2d4b7659084478e3d0a03c01b97c12f4c9eb3d6ba90a5'
starlark_fmt_sha256_aarch64_apple_darwin='188446c453f654572268734d5bc784501b975a9410efa4b0f1f0e884aa4f09e2'
starlark_fmt_sha256_x86_64_pc_windows_msvc='c9c4d71775c64ffa7989ea4fcbb63b0ac04128e62bd05b519e6bd32b9fcc52b3'
starlark_fmt_sha256_aarch64_pc_windows_msvc='a16de34be943ba1772879a84d68469019b958f2086c55dd949f0973ad39cf2ec'

reindeer_revision='3be7bd2e11d8fe949ae6db0bcbc46db8adab7264'

# reindeer does not build on stable; its rust-toolchain asks for this nightly.
# Nothing in this workspace is compiled with it.
reindeer_toolchain='nightly-2026-07-05'

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
# run. buck2 would say so only as a failed connection, well into the build.
assert_buildbuddy_key() {
    if [[ -n "${BUILDBUDDY_API_KEY:-}" ]]; then
        return 0
    fi
    echo 'buck2 runs every build on BuildBuddy, and BUILDBUDDY_API_KEY is not set.' >&2
    echo 'Set it to the organisation API key and run this again. guides/buck2.md has more.' >&2
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

# Every Starlark file a person wrote: the BUCK file beside each crate, and the
# toolchain and platform definitions. third-party/rust/BUCK is left out because
# reindeer writes it, and a formatter that rewrote it would put ./scripts/buckify
# and the check that the file is current permanently at odds.
starlark_files() {
    find "$repository/buck" "$repository/crates" "$repository/third-party/system" \
        \( -name BUCK -o -name '*.bzl' \) -print 2> /dev/null | sort
}

# Reports the Starlark files that are not formatted, and fails if there are
# any. starlark_fmt has no check mode and always exits zero, so what says a
# file would change is its diff subcommand writing one to stdout; a file that
# is already formatted writes nothing there.
check_starlark_formatting() {
    local file unformatted=()
    while read -r file; do
        if [[ -n "$(starlark_fmt --config "$repository/buck/starlark_fmt.json" diff "$file" 2> /dev/null)" ]]; then
            unformatted+=("$file")
        fi
    done < <(starlark_files)

    [[ "${#unformatted[@]}" -eq 0 ]] && return 0

    echo 'These files are not formatted. Run ./scripts/verify without --check.' >&2
    printf '  %s\n' "${unformatted[@]}" >&2
    return 1
}


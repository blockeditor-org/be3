# The rules that fetch and lay out the compilers, SDKs and sysroots the build
# uses. Each is an action run on a worker, which downloads what it names,
# checks every file against the hash pinned beside its URL, and lays it out as
# a directory. The action's key is the URLs, the hashes and the script, so a
# download is made once, by one worker, and after that it is BuildBuddy's cache
# that every build reads it from: no machine that asks for a build downloads a
# compiler or a sysroot itself.
#
# Several URLs for one hash are mirrors of the same file, tried in turn.

_DOWNLOAD = r"""
set -eu
downloads="$(mktemp -d)"
fetch() {
    sha256="$1"
    name="$2"
    shift 2
    for url in "$@"; do
        if curl --fail --silent --show-error --location --retry 5 --retry-connrefused \
            --connect-timeout 30 --speed-limit 1024 --speed-time 60 \
            --output "$downloads/$name" "$url"; then
            actual="$(sha256sum "$downloads/$name" | cut -d ' ' -f 1)"
            if [ "$actual" = "$sha256" ]; then
                return 0
            fi
            echo "$url has sha256 $actual, not $sha256" >&2
            exit 1
        fi
        echo "The download from $url did not complete." >&2
    done
    echo "Nothing could be downloaded for $name." >&2
    exit 1
}
"""

def _fetch_impl(ctx):
    if ctx.attr.directory:
        out = ctx.actions.declare_directory(ctx.attr.out or ctx.label.name)
    else:
        out = ctx.actions.declare_file(ctx.attr.out or ctx.label.name)
    fetches = []
    for sha256, urls in sorted(ctx.attr.files.items()):
        name = urls[0].rsplit("/", 1)[-1].replace("%2B", "+")
        fetches.append("fetch {} '{}' {} &".format(sha256, name, " ".join(["'{}'".format(url) for url in urls])))
    command = _DOWNLOAD + "\n".join([
        "pids=''",
        # A handful of downloads at a time, so that the sysroot's few hundred
        # packages are one action of a minute rather than several.
    ] + [
        line
        for index, fetch in enumerate(fetches)
        for line in [fetch, "pids=\"$pids $!\""] + (["for pid in $pids; do wait $pid; done; pids=''"] if index % 16 == 15 else [])
    ] + [
        "for pid in $pids; do wait $pid; done",
        'out="$1"',
        "shift",
        ctx.attr.script,
        'rm -rf "$downloads"',
    ])
    inputs = ctx.files.srcs
    script = ctx.actions.declare_file(ctx.label.name + ".fetch.sh")
    ctx.actions.write(script, command)
    ctx.actions.run(
        outputs = [out],
        inputs = inputs + [script],
        executable = "/bin/sh",
        arguments = [script.path, out.path] + [file.path for file in inputs],
        mnemonic = ctx.attr.mnemonic,
        progress_message = "Fetching %{label}",
        execution_requirements = {"requires-network": "1"},
    )
    return [DefaultInfo(files = depset([out]))]

# Downloads files (sha256 -> urls) into $downloads, named after the last part
# of the first URL, and runs script with $out, the output, and srcs as its
# arguments. With directory, the output is a directory.
fetch = rule(
    implementation = _fetch_impl,
    attrs = {
        "directory": attr.bool(default = True),
        "files": attr.string_list_dict(),
        "mnemonic": attr.string(default = "Fetch"),
        "out": attr.string(),
        "script": attr.string(),
        "srcs": attr.label_list(allow_files = True),
    },
)

def fetch_archive(name, url, sha256, strip_prefix = "", mirrors = [], **kwargs):
    """An archive, unpacked into a directory: tar's, zip or a bare zstd stream is not one."""
    archive = url.rsplit("/", 1)[-1].replace("%2B", "+")
    if archive.endswith(".zip"):
        unpack = 'unzip -q "$downloads/{}" -d "$downloads/unpacked"'.format(archive)
    else:
        unpack = 'mkdir -p "$downloads/unpacked" && tar -xf "$downloads/{}" -C "$downloads/unpacked"'.format(archive)
    fetch(
        name = name,
        files = {sha256: [url] + mirrors},
        script = "\n".join([
            unpack,
            'rm -rf "$out" && mv "$downloads/unpacked/{}" "$out"'.format(strip_prefix) if strip_prefix else 'rm -rf "$out" && mv "$downloads/unpacked" "$out"',
        ]),
        **kwargs
    )

def fetch_file(name, url, sha256, mirrors = [], executable = False, **kwargs):
    """One file, as it was downloaded."""
    file = url.rsplit("/", 1)[-1].replace("%2B", "+")
    fetch(
        name = name,
        directory = False,
        files = {sha256: [url] + mirrors},
        script = 'mv "$downloads/{}" "$out"'.format(file) + (' && chmod +x "$out"' if executable else ""),
        **kwargs
    )

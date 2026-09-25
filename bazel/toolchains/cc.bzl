load("@rules_cc//cc:action_names.bzl", "ACTION_NAMES")
load("@rules_cc//cc:cc_toolchain_config_lib.bzl", "action_config", "feature", "tool")
load("@rules_cc//cc/common:cc_common.bzl", "cc_common")
load("@rules_cc//cc/toolchains:cc_toolchain_config_info.bzl", "CcToolchainConfigInfo")

# A C toolchain made of the directories bazel/tools and bazel/sysroot fetch:
# clang-20 for one target, with its sysroot. The tools are wrapper scripts
# written next to this rule's output, which find those directories relative to
# themselves, since a directory an action made has a path only the build
# knows; the directories are built in the exec configuration, so that each is
# fetched once whatever it is compiled for.
#
# In args and link_args, {name} is the directory trees names so.

_COMPILE_ACTIONS = [
    ACTION_NAMES.assemble,
    ACTION_NAMES.preprocess_assemble,
    ACTION_NAMES.c_compile,
    ACTION_NAMES.cpp_compile,
    ACTION_NAMES.cpp_header_parsing,
    ACTION_NAMES.cpp_module_compile,
    ACTION_NAMES.cpp_module_codegen,
    ACTION_NAMES.linkstamp_compile,
]

_LINK_ACTIONS = [
    ACTION_NAMES.cpp_link_executable,
    ACTION_NAMES.cpp_link_dynamic_library,
    ACTION_NAMES.cpp_link_nodeps_dynamic_library,
]

def _relative(path, start):
    path = path.split("/")
    start = start.split("/")
    common = 0
    for index in range(min(len(path), len(start))):
        if path[index] != start[index]:
            break
        common = index + 1
    return "/".join([".."] * (len(start) - common) + path[common:])

def _quote(argument, trees):
    for name in trees:
        argument = argument.replace("{" + name + "}", "${tree_" + name.replace("-", "_") + "}")
    return '"' + argument + '"'

def _wrapper(ctx, name, program, arguments, trees, directory, trailing = [], standards = {}):
    """A script that runs program with arguments before its own and trailing after.

    standards rewrites a -std= flag the caller passes into another.
    """
    out = ctx.actions.declare_file("{}/{}".format(ctx.label.name, name))
    directory = directory + "/" + name.rsplit("/", 1)[0]
    lines = [
        "#!/bin/sh",
        'here="$(dirname "$0")"',
    ]
    for tree_name, tree in trees.items():
        lines.append('tree_{}="$here/{}"'.format(tree_name.replace("-", "_"), _relative(tree.path, directory)))
    if standards:
        # The POSIX way to rewrite "$@" without losing an argument that has a
        # space in it: take one off the front, and put it back on the end.
        lines.extend([
            "count=$#",
            "index=0",
            'while [ "$index" -lt "$count" ]; do',
            '    argument="$1"',
            "    shift",
            "    index=$((index + 1))",
            '    case "$argument" in',
        ] + [
            "        {}) argument={} ;;".format(old, new)
            for old, new in sorted(standards.items())
        ] + [
            "    esac",
            '    set -- "$@" "$argument"',
            "done",
        ])
    lines.append("exec " + " ".join(
        [_quote(argument, trees) for argument in [program] + arguments] +
        ['"$@"'] +
        [_quote(argument, trees) for argument in trailing],
    ))
    ctx.actions.write(out, "\n".join(lines) + "\n", is_executable = True)
    return out

def _clang_toolchain_config_impl(ctx):
    trees = {name: target.files.to_list()[0] for target, name in ctx.attr.trees.items()}
    trees["llvm"] = ctx.file._llvm
    directory = "{}/{}".format(ctx.bin_dir.path, ctx.label.package).rstrip("/") + "/" + ctx.label.name
    args = ctx.attr.args

    # Named like what they run, since rustc tells a linker's flavour from its
    # name; the linker is apart from the compiler because it takes arguments
    # a compile must not.
    cc = _wrapper(ctx, "bin/clang", "{llvm}/bin/clang", args, trees, directory)
    cxx = _wrapper(ctx, "bin/clang++", "{llvm}/bin/clang++", args, trees, directory, standards = ctx.attr.cxx_standards)
    if ctx.attr.linker == "lld-link":
        # After the caller's arguments: rustc runs an lld-flavoured linker with
        # `-flavor link` first, which lld only takes as the first argument.
        ld = _wrapper(ctx, "link/lld-link", "{llvm}/bin/lld-link", [], trees, directory, trailing = ctx.attr.link_args)
        ar = _wrapper(ctx, "bin/llvm-lib", "{llvm}/bin/llvm-lib", [], trees, directory)
    else:
        ld = _wrapper(ctx, "link/clang", "{llvm}/bin/clang", args + ctx.attr.link_args, trees, directory)
        ar = _wrapper(ctx, "bin/llvm-ar", "{llvm}/bin/llvm-ar", [], trees, directory)
    strip = _wrapper(ctx, "bin/llvm-strip", "{llvm}/bin/llvm-strip", [], trees, directory)
    objcopy = _wrapper(ctx, "bin/llvm-objcopy", "{llvm}/bin/llvm-objcopy", [], trees, directory)
    nm = _wrapper(ctx, "bin/llvm-nm", "{llvm}/bin/llvm-nm", [], trees, directory)

    action_configs = [
        action_config(action_name = name, enabled = True, tools = [tool(tool = cc)])
        for name in _COMPILE_ACTIONS
        if name != ACTION_NAMES.cpp_compile
    ] + [
        action_config(action_name = ACTION_NAMES.cpp_compile, enabled = True, tools = [tool(tool = cxx)]),
        action_config(action_name = ACTION_NAMES.cpp_link_static_library, enabled = True, tools = [tool(tool = ar)]),
        action_config(action_name = ACTION_NAMES.strip, enabled = True, tools = [tool(tool = strip)]),
        action_config(action_name = ACTION_NAMES.objcopy_embed_data, enabled = True, tools = [tool(tool = objcopy)]),
    ] + [
        action_config(action_name = name, enabled = True, tools = [tool(tool = ld)])
        for name in _LINK_ACTIONS
    ]

    features = [feature(name = "supports_pic", enabled = True)]
    if ctx.attr.linker == "lld-link":
        # lld-link takes none of the flags Bazel's legacy features add for a
        # gcc-style driver; rustc says everything it has to.
        features.append(feature(name = "no_legacy_features", enabled = True))
    else:
        features.append(feature(name = "supports_start_end_lib", enabled = False))

    config = cc_common.create_cc_toolchain_config_info(
        ctx = ctx,
        toolchain_identifier = ctx.label.name,
        compiler = "clang",
        target_cpu = ctx.attr.cpu,
        target_system_name = ctx.attr.target,
        target_libc = "unknown",
        abi_version = "unknown",
        abi_libc_version = "unknown",
        action_configs = action_configs,
        features = features,
        cxx_builtin_include_directories = ["/"],
    )
    files = depset([cc, cxx, ld, ar, strip, objcopy, nm] + trees.values())
    return [config, DefaultInfo(files = files)]

clang_toolchain_config = rule(
    implementation = _clang_toolchain_config_impl,
    attrs = {
        "args": attr.string_list(),
        "cpu": attr.string(mandatory = True),
        # -std=OLD: -std=NEW, for a target whose C++ library needs a newer
        # standard than a build script asks for.
        "cxx_standards": attr.string_dict(),
        "link_args": attr.string_list(),
        "linker": attr.string(default = "clang", values = ["clang", "lld-link"]),
        "target": attr.string(mandatory = True),
        "trees": attr.label_keyed_string_dict(cfg = "exec", allow_files = True),
        "_llvm": attr.label(default = "//bazel/tools:llvm", cfg = "exec", allow_single_file = True),
    },
    provides = [CcToolchainConfigInfo],
)

# The platforms that are cross-compiled from a Linux worker, and the select()
# every rule that differs between them is written with. Everything that lists
# the platforms - their targets and settings in buck/platforms/BUCK, their
# toolchains, the plans buck/cargo/buckify.bxl asks cargo for and the selects
# the generated rules pick those plans by - comes from here.
#
# Each is a target platform in buck/platforms/BUCK with a config_setting beside
# it, <name>_setting, since a select key is one setting and a platform is two
# constraints. The host - Linux on x86_64 - is DEFAULT everywhere, and the wasm
# platforms are told apart by their os constraint, which none of these share.

# name: (buck2 cpu, buck2 os, Rust triple)
CROSS_PLATFORMS = {
    "android_arm64": ("arm64", "android", "aarch64-linux-android"),
    "linux_arm64": ("arm64", "linux", "aarch64-unknown-linux-gnu"),
    "macos_arm64": ("arm64", "macos", "aarch64-apple-darwin"),
    "macos_x86_64": ("x86_64", "macos", "x86_64-apple-darwin"),
    "windows_arm64": ("arm64", "windows", "aarch64-pc-windows-msvc"),
    "windows_x86_64": ("x86_64", "windows", "x86_64-pc-windows-msvc"),
}

def cross_setting(name: str) -> str:
    return "root//buck/platforms:{}_setting".format(name)

def cross_triple(name: str) -> str:
    return CROSS_PLATFORMS[name][2]

# A select() with the host's value as DEFAULT and one entry per cross platform,
# from value(name), where a platform value() gives None for keeps the default.
def per_cross_platform(default, value, extra = {}):
    branches = {"DEFAULT": default}
    branches.update(extra)
    for name in CROSS_PLATFORMS:
        chosen = value(name)
        if chosen != None:
            branches[cross_setting(name)] = chosen
    return select(branches)

# The name cargo's plan for a platform has in buck/cargo/crates.bzl.
def plan_name(platform: str) -> str:
    return platform.replace("_", "-", 1)

# Every plan, and the setting a configuration is matched to it by. The two
# wasi plans are the same triple: the guest's setting is the app's with one
# more constraint, so select() takes it as the more specific of the two.
PLAN_SETTINGS = {
    "linux-x86_64": "root//buck/platforms:linux_x86_64_setting",
    "wasi": "root//buck/platforms:wasi_setting",
    "wasi-guest": "root//buck/platforms:wasi_guest_setting",
    "wasm32": "root//buck/platforms:wasm32_setting",
} | {plan_name(name): cross_setting(name) for name in CROSS_PLATFORMS}

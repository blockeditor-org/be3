# What the sysroots in bazel/sysroot are made of. `./scripts/bazel run
# //:lock-sysroot` resolves these into packages.bzl, the lockfile of every
# package they need, from a snapshot of the Ubuntu archive, which pins every
# index; CI fails if the lockfile disagrees with this.
SNAPSHOT = "20260915T000000Z"

WANTED = [
    # The C and C++ runtime and the gcc install clang takes them from.
    "libc6-dev",
    "libgcc-13-dev",
    "libstdc++-13-dev",
    # What third-party crates link.
    "libasound2-dev",
    "libgtk-3-dev",
    "libwebkit2gtk-4.1-dev",
    # be-compositor's session backend: displays through GBM, input through
    # libinput, a seat through libseat, and devices found through udev.
    "libgbm-dev",
    "libinput-dev",
    "libseat-dev",
    "libudev-dev",
]

# What only tests load: the Vulkan loader, lavapipe and the XKB keymaps. A set
# of its own, so that changing it leaves every compile's key alone.
TEST_RUNTIME = [
    "libvulkan1",
    "mesa-vulkan-drivers",
    "xkb-data",
]

# name: (architecture, packages)
SETS = {
    "amd64": ("amd64", WANTED),
    "amd64-test": ("amd64", TEST_RUNTIME),
    "arm64": ("arm64", WANTED),
}

load("@root//buck/platforms:cross.bzl", "CROSS_PLATFORMS")

# The release of the bootstrap for every platform a person or CI runs the
# scripts on, in cargo's release profile whatever the command asked for, laid
# out as <triple>/bootstrap(.exe). CI uploads each by its SHA-256 and pins it
# in scripts/internal/pins (buck/dev/verify.bxl's --bootstrap).

_PLATFORMS = {
    "linux_x86_64": "x86_64-unknown-linux-gnu",
} | {name: triple for name, (_, os, triple) in CROSS_PLATFORMS.items() if os in ["linux", "macos", "windows"]}

def _released(name: str):
    def impl(platform: PlatformInfo, refs: struct) -> PlatformInfo:
        target = getattr(refs, name)[PlatformInfo]
        release = refs.release[ConstraintValueInfo]
        constraints = dict(target.configuration.constraints)
        constraints[release.setting.label] = release
        return PlatformInfo(
            configuration = ConfigurationInfo(constraints = constraints, values = target.configuration.values),
            label = target.label + "_release",
        )

    return transition(
        impl = impl,
        refs = {name: "root//buck/platforms:" + name, "release": "root//buck/constraints:release"},
    )

_linux_x86_64 = _released("linux_x86_64")
_linux_arm64 = _released("linux_arm64")
_macos_x86_64 = _released("macos_x86_64")
_macos_arm64 = _released("macos_arm64")
_windows_x86_64 = _released("windows_x86_64")
_windows_arm64 = _released("windows_arm64")

def _dist_impl(ctx: AnalysisContext) -> list[Provider]:
    files = {}
    for name, triple in _PLATFORMS.items():
        binary = getattr(ctx.attrs, name)[DefaultInfo].default_outputs[0]
        executable = "bootstrap.exe" if name.startswith("windows") else "bootstrap"
        files[triple + "/" + executable] = binary
    return [DefaultInfo(default_output = ctx.actions.copied_dir("dist", files))]

bootstrap_dist = rule(
    attrs = {
        "linux_arm64": attrs.transition_dep(cfg = _linux_arm64),
        "linux_x86_64": attrs.transition_dep(cfg = _linux_x86_64),
        "macos_arm64": attrs.transition_dep(cfg = _macos_arm64),
        "macos_x86_64": attrs.transition_dep(cfg = _macos_x86_64),
        "windows_arm64": attrs.transition_dep(cfg = _windows_arm64),
        "windows_x86_64": attrs.transition_dep(cfg = _windows_x86_64),
    },
    impl = _dist_impl,
)

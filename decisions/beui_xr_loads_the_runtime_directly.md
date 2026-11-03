# beui-layer-xr can load the OpenXR runtime without a system loader

beui-layer-xr loads OpenXR through the `openxr` crate's `loaded` feature: the system's `openxr_loader` when there is one, else the active runtime's own library, found as the Khronos loader finds it (`XR_RUNTIME_JSON`, `active_runtime.json` under the XDG config directories on Linux, the `ActiveRuntime` registry value on Windows) and negotiated with directly (`runtime.rs`).

## Why

Windows has no system OpenXR loader and SteamVR ships none, so relying on a loader would leave headsets unreachable there.

## Rejected

- Requiring the system `openxr_loader`: absent on Windows and not shipped by SteamVR.

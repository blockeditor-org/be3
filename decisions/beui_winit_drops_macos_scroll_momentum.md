# beui-platform-winit drops macOS scroll momentum events

On macOS, `beui-platform-winit` (`src/wheel.rs`) drops the momentum wheel events the system sends after the fingers lift from a trackpad (from a `Started` phase within 100 ms of the lift to their `Ended`). The lift is reported as `Input::ScrollEnded`, and beui's own `Scroll` springs back or flings from there.

## Why

beui's fling replaces the system momentum, and the momentum events would otherwise scroll clamped at an end instead of stretching and banding past it.

## Rejected

- Passing the system's momentum events through as ordinary wheel scrolls: they clamp at an end instead of banding.

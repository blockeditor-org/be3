# Product-specific components live in the product, not in beui

If one crate is the only possible user, the component lives in that crate. `KeepChanges`, `LockScreen`, `CrashNotice` and `StaleNotice` moved to be-host; the level OSD and the launcher's program items and "Run" row to the desktop app; `ThemeSelect` and `MotionSelect` into the settings editor; `PreferencesSection` and `Bordered` were deleted. The dock and the software keyboard became their own crates (`beui-dock`, `beui-keyboard`), each with `unstyled` and `styled` modules, re-exported as `beui::dock` and `beui::keyboard`. `styled::Stack` stays in styled because its breakpoint is a style choice.

## Why

These components' text and behaviour were about one product (apps crashing, display settings, the lock screen), and each had a single user. The dock and keyboard are generally useful, so they stay in beui but in crates any beui user can style themselves; beui-inspector was the precedent for a beui crate outside the component crates.

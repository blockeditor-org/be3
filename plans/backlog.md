# Backlog

The running to-do list, grouped. Every item says what "done" looks like.
Items marked **(ask)** need a decision from the user during the work. Fixed
items are removed as they land.

## 1. Android startup crash
The app is expected to crash on startup on Android. The emulator can't run in
the cloud containers (no KVM, see guides/running_on_android.md), so this needs
a machine with KVM or a device.

## 2. beui scrolling

- **First scroll is short on Linux.** The first wheel event scrolls less than
  every later one.
- **Edge fade.** The styled scroll view fades content out with a short
  opacity gradient at clipped edges instead of a hard cut.

## 3. Tooling and repo hygiene

- **Crate folder names use `-`.** Rename every crate folder under `crates/`
  that uses `_` (`beui_macros`, `reactive_macros`, `tabletop_games` and its
  rules, the `crates/editors/*` folders).

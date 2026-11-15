# KeepAlive's `awake` keeps a joined dock's holder running

`KeepAlive` takes an `awake` key set besides `keep`: a key it lists keeps its subtree running
while no `Kept` shows it, and `kept_shown()` inside reads true. The dock lists in it the tabs whose
own dock joined it (`DockConfig::hosts`). Every other hidden panel pauses.

## Why

- Once a tab's own dock joins the dock around it, the outer layout replaces the tab by the inner
  dock's group, so no pane shows the holder tab's panel. That panel is the content whose dock
  joined (an app child frame, a `Docking` in a `DockTab`), and its effects must keep running: the
  inner dock reacts to its state, closes its tabs and restores its layout from there.
- Its `kept_shown()` must read true, or the join lets the tab go (guides/beui/docking.md), the
  holder shows again, joins again, and the two docks loop.
- One extra key set on `KeepAlive` says exactly that: shown elsewhere, so running and shown.

## Rejected

- **The inner dock shows the holder's panel through a `Kept` of its own.** Every place to show it
  is either laid out, which places the app frame there and resizes the app to that rect (even a
  zero-size or offscreen one), or not laid out, which is "hidden but running" by another name.
- **A dock-local `kept_shown()` for every panel** (each panel reading the dock's own shown-ness).
  A panel hidden behind another tab would then read as shown, and what it holds outside itself
  would not let go; only the holders need it, which `awake` says.
- **Leaving the holders out of the `KeepAlive`** (a hand-written cache for them alone): two ways to
  keep a panel in one dock.

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

## The dock's panels read the dock's own `kept_shown()`

A panel's `kept_shown()` is the dock's own, not its `Kept`'s. What a tab's content holds outside
itself is a dock joined into this one, whose group lives in this dock's layout, so a pane showing
another tab, or building its view again, must not make it let go: be-sdk-beui's
`a_root_hosts_a_childs_dock_again_after_the_child_withdraws_and_redeclares_it` saw a holder's
`Kept` released one change before the new one took it (the pane's view built again), and the join
let go in between. `awake` still keeps a holder running; this keeps its join. A panel hidden
behind another tab reading as shown lets nothing go that should: a tab whose dock joined leaves
the layout as itself (it is a holder, never a tab hidden behind another), and every other hidden
panel is paused, so nothing in it acts on what it reads.

## Rejected

- **The inner dock shows the holder's panel through a `Kept` of its own.** Every place to show it
  is either laid out, which places the app frame there and resizes the app to that rect (even a
  zero-size or offscreen one), or not laid out, which is "hidden but running" by another name.
- **Leaving the holders out of the `KeepAlive`** (a hand-written cache for them alone): two ways to
  keep a panel in one dock.

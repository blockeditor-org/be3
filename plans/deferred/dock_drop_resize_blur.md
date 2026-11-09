# Blurring a dock pane across its resize after a drop

Depends on: plans/declarative_dragging.md, step 3.

During a fluid dock drop, panes show their panels clipped at the size they were laid out at, and
resize them for real only when the drop commits. The content then jumps to its new layout,
possibly a frame or more later for an app or Wayland program that has to answer the new size.
Hide that jump the way iOS does: when a pane's size changes at a drop, blur its panel lightly from
the release, keep it blurred until the panel has drawn at its new size (for an app, its first
present at the new configure serial; guides/apps/runtime.md), then unblur it.

What it needs:
- **A blur on a subtree.** Today beui's blur is the filter pass over the whole frame
  (`Context::apply_filter`, `Group::Filter`, a dual Kawase chain:
  decisions/beui_filter_blur_is_dual_kawase.md), used by the inspector. A pane needs it on one
  subtree, drawn offscreen and blurred, with damage widened by the chain's known reach. This shares
  its offscreen group with true group opacity (plans/styled_components.md); the HTML core can use
  CSS `filter: blur()`.
- **A signal that the panel has redrawn at its new size**, so the unblur waits on it rather than
  on a timer. Panels drawn by beui alone have redrawn by the next frame.
- **A token for the blur radius and its timing**, and no blur under `Motion::Instant`.

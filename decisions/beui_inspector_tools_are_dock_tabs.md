# The inspector's tools and the inspected app are tabs of a dock

beui-inspector's Beui, Comp, A11y, Perf and Sim tools are `DockTab`s of one pane in a `Docking` under a styled `DockHost`, and when it embeds a program, the app is a `see_through` `Program` tab in its own pane beside them. The tree tools pin a Pick toggle to the dock's bar. Its layout is not saved, like the rest of the inspector's state.

## Why

The tools are independent panels, and side by side is useful. The dock treats the inspector like any other dock (in a narrow panel it stacks), so the compact App/Inspector bar is the dock's own phone layout. A pinned Pick action replaces a toggle above the dock that came and went as tools were switched.

## Rejected

- A `stack_below=0.0` prop on the styled `DockHost` to keep the inspector tiled: it was a special mode for the inspector, and was removed.

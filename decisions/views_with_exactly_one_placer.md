# An editor draws views, and every view has exactly one placer

An editor instance draws views, each with an id unique in the instance (view 0 is its main view). Frame, tab, bar body, window, display, preview, artifact settings and anchored view are not region kinds with their own features: the menu, bar actions, child placements, the camera, anchored views, chrome, keyboard requests, intercepted keys and accessibility belong to the view. Who places a view is separate from the view: its parent (a child placement), the root dock (a tab or bar body), the host (a window or a display), or the view itself against an anchor in its parent view.

## Why

- Docking is then only a question of who places the views: the canvas joining the workspace moves its stage view and bar view to the root's placement, and no feature is lost because features follow the view.
- An editor creates any view it needs in its own commit and the host places it in that frame, so opening a window costs no frame either.
- Popping out a dock tab keeps the root as its placer: the root holds the only dock state, so it draws the dock window's chrome in a window view it creates with no frame of delay, and places the tab's body in it.

## Rejected

- A request to change a view's placer when popping out: not needed once the root draws popped-out windows itself.

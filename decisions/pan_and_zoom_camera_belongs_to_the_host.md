# A pan-and-zoom editor's camera, and keeping it fitted, belong to the host

An editor with `capabilities.pan_and_zoom` keeps no offset or zoom of its own. The host hands it a camera (`editor.canvas()`, `editor.scale()`), the editor asks for moves (`pan`, `zoom`, `fit`, `reveal`), and an editor that should open showing its content declares what to fit once with `editor.keep_fitted`, after which the host does the fitting whenever it lays the frame out. Whether the camera has been moved away from its fit is kept by the host's camera, per block, for as long as be-host runs. See guides/pan_and_zoom.md.

## Why

- The same editor code works in a tab of its own and as a card inside another editor's canvas without knowing which. "Zoom in" means the same thing whether it came from the editor's own button or a wheel over it on an infinite canvas, and a zoom a focused child asks for moves the camera the canvas is drawn through.
- Fitting is declared rather than sent as moves: a fit the editor worked out from the view it had been sent was stale whenever the frame resized over several frames, and the moves added up. The host re-fits in the same frame the content band, the world or the chrome changes, with nothing sent to the editor and back.
- `moved` is not kept in the editor's view block, because that block outlives the camera it describes: be-host restarted on a remembered `moved` would open on the default view, neither fitted nor where the person left it, and a tab opened again gets a new camera anyway.

## Rejected

- The editor sending the fit as pan and zoom moves: stale during multi-frame resizes, and the moves accumulated.
- Storing `moved` in the editor's view block: it outlives the camera, so a restart would open on a view that is neither fitted nor the person's.

# A region move never changes a pan-and-zoom camera

When the host moves a region that holds a pan-and-zoom editor, the camera is unchanged: the content rect is centred on the content area's centre plus `pan`, so a moved region carries its world. A resize keeps the centre of the content area once the camera was moved, and keeps the content fitted before that. No editor declares an anchor of its own. The frame report gives the content area as insets from the region's edges, so the host computes the new content area during a resize itself and sends the new view in the same frame.

## Why

- Moving a region should carry what it shows with it, not pan the world the other way.
- Insets rather than a rectangle let the host lay out the content area without waiting for the app, so a resize has no frame of lag. Only a frame where the app changes its own insets (a sidebar moving into a sheet when the frame narrows) lags one frame.

## Rejected

- Adding the opposite of a region's move to `pan` (the old `state.center` correction in `BlockFrameView`).

# Our own cursors where we draw the cursor, the system's wherever there is one

Where a platform shows a cursor of its own (winit, Windows, the web, Android), beui asks it for
the `CursorIcon` and it draws its own. Where nothing else can draw one, beui draws
`beui_core::drawn_cursor`'s icons: the DRM platform in a login session, the space layer, mouse
simulation and touch emulation. Programs under be-host-wayland keep their own cursors: what
they draw through `wl_pointer.set_cursor` is shown as is, and a `wp_cursor_shape_v1` shape is
drawn as ours.

## Why

One look in everything beui draws, at any scale and on any display, with nothing to install.
In a login session be-host is the desktop, so the cursor there is ours to define.

## Rejected

- Loading the system's XCursor theme in the DRM platform (and `XCURSOR_THEME`): a second way to
  draw a cursor, whose look depended on what the machine had installed, and which fell back to
  a hand-drawn arrow without one.
- Importing the system's cursor images on every platform for the space layer and simulation:
  the web and Android cannot hand them over, so `drawn_cursor` would still be needed; bitmaps
  blur when a panel is scaled; and paintings would depend on the machine.
- Telling the programs the compositor starts to use a theme made from our icons (dconf's
  `cursor-theme`, `XCURSOR_PATH`): their look belongs to a distribution, which sets it for every
  toolkit at once, including programs we do not start. The compositor changes what programs do
  only where their window would show controls that repeat ours, as the dconf profile that hides
  GTK's window buttons does (be-host-wayland's `gtk_settings.rs`).

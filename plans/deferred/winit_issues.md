# winit gaps

Depends on: none.

What beui needs from winit that it lacks, each to be added upstream where it can be, or worked
around in crates/beui-platform-winit until then.

- **A window's background colour.** Between a window growing and the program drawing at the new
  size, the system shows something in the new part: black on X11 unless the window's background is
  set, and black or white on Windows, where the swapchain falls behind DWM's resize. Root's clear
  colour belongs there. beui sets it on X11 alone, through its own connection
  (crates/beui-platform-winit/src/backdrop.rs). On Windows the same takes about 20 lines of
  Win32: a class brush, or answering `WM_ERASEBKGND` by filling the client area with the clear
  colour, set again each time the colour changes. A `Window::set_background_color` in winit would
  replace both. How much of the gap it covers on Windows also depends on how the swapchain
  presents (plans/deferred/dxgi_scaling_stretch.md).

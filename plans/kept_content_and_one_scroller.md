# Kept content is the caller's choice, and a sheet scrolls with Scroll

Depends on: none.

Two places in beui where a component does by hand, and differently each time, what a base
piece should do once, plus small duplicates found beside them.

## Steps

Steps 1 (a kept subtree reads the context of the `Kept` showing it), 2 (overlays build their
body on each open; keeping it is the caller's `Kept`), 3 (`Scroll`'s `snaps`) and 4 (a sheet is
one `Scroll`, with sticky items and `bar_from`, running beui's own motion on both cores) have landed.

5. **One card surface.** `Card`, `DialogCard` and `PopoverPanel` (styled `card.rs`,
   `dialog.rs`, `popover.rs`) are the same frame (Surface fill, the default border, `Lg`
   radius) with different padding and width. One `Card` with a maximum width and an optional
   beak.
6. **One colour wheel.** styled `color_wheel/cpu.rs` (a cached image) and `gpu.rs` (a wgpu
   pipeline) sit behind beui-components-styled's `gpu` feature, which only the native core
   turns on. Measure the cached image at the largest size the demo shows; if it holds a frame,
   both cores use it, and `gpu.rs` and the crate's optional wgpu, bytemuck and beui-draw
   dependencies go.
7. **Shared finders for tests.** `text_within` is written in beui-host's, beui-layer-space's,
   be-app-workspace's, be-app-desktop's, be-app-settings' and be-app-terminal's tests and as
   beui-demo's `find_text`, some matching exactly and some by substring, and beui-layer-space's
   tests copy beui-host's `build_node`, `dock_tab`, `windowed_dock` and `PanZoomStage`. One
   `find_text(document, root, text, Match)` and the dock fixtures in
   `beui_platform_headless::testing`; the copies go.

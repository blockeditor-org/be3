# Kept content is the caller's choice, and a sheet scrolls with Scroll

Depends on: none.

Two places in beui where a component does by hand, and differently each time, what a base
piece should do once, plus small duplicates found beside them.

## Steps

1. **A kept subtree keeps its context.** Content kept while an overlay is closed does not see
   the overlay's presence unless its wrapper passes it in by hand (`presence.provide()` inside
   `<Kept>`), so `Modal` (modal.rs), `Anchored` (anchored.rs) and `DockDialog`
   (beui-dock-host's `unstyled/dialog.rs`) each wrap their body the same way, and forgetting
   the call breaks presence inside the kept body. `Kept` captures the context it was built in,
   the overlay's presence included, and gives it to its subtree whenever it is shown.
2. **Keeping content is the caller's choice.** `Modal` and `DockDialog` always keep their body,
   `Anchored` has a `keep_alive` prop. All three build their body on open and drop it on close;
   a caller that wants its form's state kept while closed wraps its own body in `<Kept>`, which
   step 1 makes correct. `Anchored`'s `keep_alive` and the three wrappers go, as do the
   "ever opened" flags in `ContextMenu` and `Dialog`. A body built fresh on each open also
   cannot show values left over from the last one.
3. **Scroll snaps.** `Scroll` gets snap stops along an axis: offsets a fling and a release
   settle on. A carousel or a pager uses them too.
4. **A sheet is one Scroll.** sheet.rs drives its content's offset itself, with its own wheel
   handling, fling and bounce phases and a `banding()` that repeats scroll.rs's
   `rubber_banding()`, so a sheet's content has no scrollbar, keyboard scrolling or autoscroll.
   A sheet becomes a vertical `Scroll` whose content is the space above the sheet and the sheet
   itself, with snap stops at the sheet's rest positions: its one offset is both how far the
   sheet is raised and how far its content has scrolled, so pulling up raises the sheet until
   it is open and then scrolls its content. Sheet keeps only its stops and dismissing when
   pulled below the lowest. Its own wheel, fling, bounce and `banding()` go. This is the model
   plans/deferred/html_core.md wants on the web (the stops as CSS scroll snap), so both cores
   share one design.
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

## The test

- A dialog whose kept body reads the overlay's presence fades with it, with no `provide()`
  anywhere.
- A sheet's content longer than the sheet shows a scrollbar and scrolls from the keyboard; a
  pull from the content's top lowers the sheet, and a pull below the lowest stop dismisses it.
- The demo's sheet and dialog paintings are unchanged apart from the scrollbar.

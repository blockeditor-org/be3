# A perfect frame when a plugin resizes another

Not started. When one plugin moves or resizes a region that another plugin draws (workspace-ui's splitter between Files and an editor, a canvas card holding a checklist, a dock window resized by its border), the child should show its content laid out at the new size in the very frame the parent shows its new layout. Today it lags one or two frames, and in those frames the host shows the child's last frame at its own size, anchored to the top left of the new rect (`presenter::unstretched`, chosen from the size the host just asked for, `Instances::request_of`), with whatever the parent painted around it.

## Why it lags

Follow a drag of workspace-ui's splitter, which is a plugin hosting file-tree as a child:

1. Pass N: the host forwards the pointer move to workspace-ui (`InputBatch`). workspace-ui handles it, lays out, paints its frame and commits it (`frame-commit`), with the new `children` placements in it. The host waits for that commit within `FRAME_BUDGET` (`runtime::settle`).
2. The host only learns Files' new rect from that commit, after it has already sent this pass's `Screens` to file-tree (`Runtime::update`, at `begin_frame`). So Files hears its new size at pass N+1, and its frame at that size arrives in N+1 at best, N+2 if it misses the budget.
3. In the meantime pass N shows workspace-ui's new layout around Files' old frame.

The order of work inside a plugin is the cause: input, layout and painting run as one step, so the parent's layout reaches the host only together with its finished painting, too late for the child to paint in the same pass.

## Design: split interaction from rendering

Give a plugin's frame two phases in the protocol, each answered on its own:

1. **Interact.** The host sends a pass's input and the screens it already knows. The plugin applies the input, runs its layout and answers with a `layout-commit`: everything the host needs to place things, with no painting. That is the part of `frame-commit` that is geometry: `sizes`, `children` placements, the content insets, cursor, claims, floating and painted rects, scrims, anchored layer placements, window requests, dock messages. It carries the `serial` of the screens it was laid out against.
2. **Render.** The host sends `render(pass)` with the final screens for every region of that plugin, now including sizes its parents just decided. The plugin paints into its surfaces (textures stay on the GPU, as now) and answers with `presented`, tagged with the pass.

The host runs the phases per pass, parents before children:

- Deliver input and collect `layout-commit`s from every plugin that had input or a due frame, within a share of the budget. A parent's commit gives its children their new rects; the host updates their `ScreenRequest`s at once (`apply_offers`) instead of at the next `begin_frame`.
- Children laid out by a parent's commit in this pass go through their own interact phase now if their size changed (they may have children of their own: repeat down the tree, bounded by depth).
- Then ask every plugin to render, and wait for the presents within the rest of the budget.
- Present the pass as one transaction: a parent's new frame is shown together with its children's frames at the sizes it gave them. A region whose present did not arrive in time keeps its last frame, unstretched and anchored top left as today. The parent still shows its new frame: holding the parent back for a slow child would make the whole dock stutter, and the unstretched fallback is good enough for a frame or two.

Keep the protocol free of any GUI framework: the phases are "apply input and lay out" and "paint what you laid out". beui maps them to its own input + layout pass and its paint pass; an egui plugin can run its input and layout (`Context::run` without tessellating) in the first and paint in the second, or do both in the first and only present in the second. A plugin that does not split its work answers interact with an empty layout commit and does everything in render, which is today's behaviour.

## Where a plugin still misses the frame

- A child that takes longer than the budget to paint (a large PDF page, a scene) shows its last frame unstretched until it catches up. Nothing can fix that short of the host scaling the old frame, which is the stretching this plan replaces.
- A child that resizes because of its own content (intrinsic size) still needs a round trip: its parent hears the size in its next pass.
- A plugin whose worker is busy (a long synchronous call) answers neither phase in time; the pass shows its last frame.
- On the web the phases cross a worker boundary each way; the budget may only cover one of them for nested plugins, so web may stay one frame behind for grandchildren.

## Steps

1. Protocol: add `layout-commit` and `render` to `crates/block-plugin-api/wit/types.wit`, move the geometry out of `frame-commit` into `layout-commit`, and tag presents with the pass they render. Update guides/adding_a_plugin_editor.md.
2. Plugin SDK (`crates/block-editor-plugin`, `crates/beui-platform-plugin`): run beui's input and layout on interact, paint on render. Test that a parent's layout commit arrives before it paints.
3. Host (`crates/block-app-plugin-host/src/runtime.rs`): run the two phases in `poll`/`settle`, parents before children, and update child requests from a parent's layout commit in the same pass. Test with two fake plugins that a child resized by its parent's commit is asked to render at the new size in that pass.
4. Compositor: present a pass's frames together; keep the unstretched fallback for regions that missed it. An app test drags workspace-ui's splitter with the clock stopped and checks every frame shows the Files content laid out at the size of the frame around it.

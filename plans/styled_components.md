# Styled component cleanup: what is left

Depends on: none.

The styled component cleanup (behaviour moved into `beui-components-unstyled`,
product components moved out of beui, merged components, design tokens, one
text `overflow` prop, modal actions with roles, code outside beui on the
styled layer) is done. This item remains.

## True group opacity

`Frame`'s `opacity` (`set_frame_opacity`) drives beui-core's fade paint state
(`beui-core/src/fade.rs`), which multiplies each shape's coverage, so
overlapping shapes inside a faded subtree show through each other natively.
The level OSD, the dock's drop highlight and drag preview, and the split
button's divider use it. The HTML core already fades the group as a whole
(CSS `opacity`). Natively, composite the faded subtree offscreen first
(beui-renderer-wgpu, with a group in `beui-paint-format`) and fade the result.
Damage still comes from the retained tree by identity. Done: a painting of
two overlapping shapes in a faded frame shows no show-through.

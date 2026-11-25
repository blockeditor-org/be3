# The colour wheel draws on the gpu where it can

beui-components-styled's colour wheel keeps two faces: `color_wheel/gpu.rs`, a wgpu pipeline the
native core uses (the crate's `gpu` feature, which beui turns on there), and `color_wheel/cpu.rs`,
an image computed on the CPU and cached until the hue or size changes, which the HTML core uses.

## Why

The cached image is computed again on every frame the hue changes, as dragging the ring does, and
it does not fit in a frame at the size the demo shows (`WHEEL_SIZE`, 200 points). Measured
natively in a release build on an x86-64 build worker, the median of ten hues:

| Model | 1x | 2x | 3x |
| --- | --- | --- | --- |
| HSV | 1.7 ms | 6.4 ms | 14.2 ms |
| OKLCH | 13.5 ms | 49.5 ms | 123 ms |
| OKLCH, `max_chroma` from a 512-entry table per hue | 4.8 ms | 18.0 ms | 41.1 ms |

A phone at 3x would miss several frames for each step of a drag even with the table, before
counting a phone's slower CPU or wasm. The gpu face draws the same pixels in a shader at any
scale for nothing on the CPU.

## Rejected

- **The cached image on both cores**, dropping `gpu.rs` and the crate's optional wgpu, bytemuck
  and beui-draw dependencies. A plan proposed it on the condition that the image held a frame,
  which it does not.

# Space keeps one renderer per window and per cursor sprite

In beui-layer-space, each window's display is drawn by its own `beui_renderer_wgpu::Renderer`. When a window changes size it gets a new texture but keeps its renderer, and the cursor sprite repaints a new icon into the same texture with the same renderer.

## Why

A renderer's buffers and encoded display lists cost far more to build than a resize step's whole frame, and building a renderer for each cursor icon cost about 30 ms a switch.

## Rejected

- A new renderer per resized texture or per cursor icon: too slow (about 30 ms per cursor icon switch; more than a whole resize frame).

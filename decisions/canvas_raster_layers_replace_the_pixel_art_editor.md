# Raster layers in the canvas replace the pixel-art editor

The pixel-art editor was removed; its tools and operations moved into the infinite canvas's Raster profile, where a raster layer is an entity of the canvas (`CanvasShape::Raster`). A game level made of pixel art is a canvas holding such layers, or child editors.

## Why

One editor for 2D work, with every profile over the same document, rather than a separate editor per kind of image.

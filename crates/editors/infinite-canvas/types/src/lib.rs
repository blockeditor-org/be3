mod canvas;
mod raster;

pub use canvas::{
    BINDING_GAP, Canvas, CanvasArrowhead, CanvasBinding, CanvasColor, CanvasComponent,
    CanvasContent, CanvasEntity, CanvasFill, CanvasGrid, CanvasGridSpacing, CanvasHue,
    CanvasLayerMove, CanvasLook, CanvasPathPoint, CanvasPoint, CanvasProfile, CanvasShape,
    CanvasStrokeStyle, CanvasStyle, CanvasTextAlign, CanvasTextSize, CanvasTextStyle,
    CanvasTextVerticalAlign, CanvasTextWeight, CanvasTone, CanvasTransform, Component, Entity,
    InfiniteCanvasOperation, normalized_entity,
};
pub use raster::{
    CanvasPixel, CanvasPixelUpdate, CanvasRaster, CanvasRasterAnchor, DEFAULT_PIXEL_SIZE,
    DEFAULT_RASTER_SIZE, MAX_RASTER_PALETTE_COLORS, MAX_RASTER_SIZE, default_raster_palette,
    pixel_size_or_default, raster_dimensions, snap_to_pixels,
};

block_type_guest::be_block_types![CanvasContent { undo }];

#[cfg(feature = "module")]
block_type_guest::type_module!(BlockTypes);

#[cfg(test)]
mod tests;

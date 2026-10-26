mod canvas;

pub use canvas::{
    BINDING_GAP, Canvas, CanvasArrowhead, CanvasBinding, CanvasColor, CanvasComponent,
    CanvasContent, CanvasEntity, CanvasFill, CanvasGrid, CanvasGridSpacing, CanvasHue,
    CanvasLayerMove, CanvasLook, CanvasPathPoint, CanvasPoint, CanvasProfile, CanvasShape,
    CanvasStrokeStyle, CanvasStyle, CanvasTextAlign, CanvasTextSize, CanvasTextStyle,
    CanvasTextVerticalAlign, CanvasTextWeight, CanvasTone, CanvasTransform, Component, Entity,
    InfiniteCanvasOperation, normalized_entity,
};

block_type_guest::be_block_types![CanvasContent { undo }];

#[cfg(feature = "module")]
block_type_guest::type_module!(BlockTypes);

#[cfg(test)]
mod tests;

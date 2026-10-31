use be_block::{BlockContent, Edit, LiveEdit, Merge, Undo};
use be_commit::MergeResult;
use database_types::database::DatabaseValue;
use uuid::Uuid;

use crate::{
    Canvas, CanvasContent, CanvasEntity, CanvasPixel, CanvasPixelUpdate, CanvasPoint, CanvasShape,
    CanvasTransform, InfiniteCanvasOperation,
};

mod a_canvas_entity_removed_on_one_side_and_moved_on_the_other_stays_removed;
mod a_canvas_profile_is_kept_but_never_undone;
mod a_component_added_to_one_entity_on_both_sides_is_kept_once;
mod a_pixel_painted_outside_the_other_sides_crop_counts_as_a_conflict;
mod a_raster_layer_sits_on_the_canvas_wide_pixel_grid;
mod an_arrow_end_bound_to_a_shape_follows_it_and_stays_when_it_is_removed;
mod canvas_children_are_removed_repointed_or_merged;
mod canvas_layers_reorder_and_keep_unlisted_slots;
mod canvases_merge_a_move_and_a_restyle_of_one_entity;
mod entities_brought_forward_on_both_sides_merge_to_both_moves;
mod raster_layers_fill_resize_and_keep_a_paint_made_during_a_resize;
mod resizing_a_raster_layer_keeps_its_pixels_where_they_were;
mod text_retyped_in_one_canvas_entity_by_two_peers_keeps_both;
mod two_peers_restyling_different_fields_of_one_shape_keep_both;
mod undoing_a_clear_keeps_a_pixel_painted_since;

fn edited<C: LiveEdit + Clone>(start: &C, edits: impl IntoIterator<Item = C::Op>) -> C {
    let mut content = start.clone();
    for edit in edits {
        content.apply(&edit);
    }
    content
}

fn merged<C: Merge>(base: &C, ours: &C, theirs: &C) -> (C, usize) {
    match C::merge3(base, ours, theirs) {
        MergeResult::Clean(value) => (value, 0),
        MergeResult::Conflicted { value, conflicts } => (value, conflicts),
    }
}

fn rectangle() -> CanvasEntity {
    CanvasEntity::new(
        CanvasShape::Rectangle,
        CanvasTransform::new(CanvasPoint::default(), CanvasPoint::new(10.0, 10.0), 0.0),
    )
}

fn canvas_run(content: &CanvasContent, operation: InfiniteCanvasOperation) -> CanvasContent {
    edited(content, [Canvas::edit_in(content, &operation)])
}

const RED: CanvasPixel = CanvasPixel::new(255, 0, 0, 255);
const BLUE: CanvasPixel = CanvasPixel::new(0, 0, 255, 255);

fn raster_layer(content: &CanvasContent, width: u16, height: u16) -> (CanvasContent, Uuid) {
    let layer = CanvasEntity::raster(
        Canvas::pixel_size_in(content),
        CanvasPoint::default(),
        width,
        height,
    );
    let id = layer.id;
    let content = canvas_run(
        content,
        InfiniteCanvasOperation::Add {
            entity: Box::new(layer),
        },
    );
    (content, id)
}

fn paint(content: &CanvasContent, id: Uuid, x: u16, y: u16, color: CanvasPixel) -> Edit {
    Canvas::edit_in(
        content,
        &InfiniteCanvasOperation::Paint {
            id,
            pixels: vec![CanvasPixelUpdate { x, y, color }],
        },
    )
}

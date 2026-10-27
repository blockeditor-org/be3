use crate::primitives::*;
use crate::reactive::*;
use crate::types::*;
use beui_macros::view;
use paint_snapshot::BrowserSnapshot;

const INK: Color32 = Color32::from_rgb(30, 30, 40);
const PAPER: Color32 = Color32::from_rgb(250, 250, 247);
const BLUE: Color32 = Color32::from_rgb(70, 110, 220);
const GREEN: Color32 = Color32::from_rgb(60, 170, 110);
const AMBER: Color32 = Color32::from_rgb(235, 170, 50);

fn painting(size: [u32; 2], root: impl FnOnce() -> Root) -> BrowserSnapshot {
    let markup = crate::running::render_to_string(root);
    let page = format!(
        "<!doctype html><html><head><meta charset=\"utf-8\"><style>{}body {{ margin: 0; height: 100vh; font: 14px sans-serif; }}</style></head><body>{markup}</body></html>",
        crate::running::BASE_STYLES
    );
    BrowserSnapshot::new(size, &page)
}

fn swatch(color: Color32, width: f32, height: f32) -> NodeId {
    view! {
        <Frame width={width} height={height} fill={color} />
    }
}

mod a_canvas_pans_and_scales_its_items;
mod a_drawing_is_a_pink_box_of_its_size;
mod a_frame_moves_turns_and_fades_what_it_holds;
mod a_frame_sizes_pads_fills_rounds_and_aligns_what_it_holds;
mod a_grid_shares_its_columns_between_its_cells;
mod a_list_lays_out_its_children_in_a_line_by_their_sizing;
mod a_portal_shows_a_node_built_elsewhere;
mod a_virtual_list_builds_the_rows_its_first_reach_covers;
mod an_interactive_decorates_the_element_of_what_it_holds;
mod an_offset_clips_its_child_to_its_box;
mod an_overlay_anchors_its_content_to_a_node_and_points_at_it;
mod layers_stack_their_children_over_one_box;
mod text_shows_glyphs_over_the_bytes_they_cover;
mod text_wraps_cuts_and_styles_its_spans;

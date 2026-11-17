use super::*;
mod a_beside_overlay_lines_up_with_its_anchor_on_every_side_and_alignment;
mod a_beside_overlay_flips_at_each_window_edge;
mod a_beside_overlay_shifts_along_its_side_to_stay_in_the_window;
mod above_or_below_an_overlay_fitting_neither_side_takes_the_roomier_and_shrinks;
mod left_or_right_an_overlay_fitting_neither_side_keeps_its_width_and_shifts_over_its_anchor;
mod a_beside_overlays_gap_and_margin_apply_on_every_side;
mod the_room_beside_an_anchor_is_what_an_overlay_shrinks_to;

const VIEWPORT: Rect = Rect {
    min: Pos2::new(0.0, 0.0),
    max: Pos2::new(400.0, 300.0),
};

const SIDES: [AnchorSide; 4] = [
    AnchorSide::Above,
    AnchorSide::Below,
    AnchorSide::Left,
    AnchorSide::Right,
];

const ALIGNS: [AnchorAlign; 3] = [AnchorAlign::Start, AnchorAlign::Center, AnchorAlign::End];

fn rect(left: f32, top: f32, right: f32, bottom: f32) -> Rect {
    Rect::from_min_max(pos2(left, top), pos2(right, bottom))
}

fn placement(side: AnchorSide, align: AnchorAlign) -> Placement {
    Placement::Beside {
        side,
        align,
        gap: 0,
        margin: 0,
    }
}

fn placed(placement: Placement, anchor: Rect, width: f32, height: f32) -> Rect {
    resolve_rect(VIEWPORT, anchor, placement, Vec2::new(width, height))
}

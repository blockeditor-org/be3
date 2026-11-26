use super::*;
use be_sdk_beui::beui::{PointerButton, vec2};

#[test]
fn a_carried_display_reaches_the_areas_far_corner() {
    let displays = laptop_beside(gaming_monitor());
    let mut settings = Harness::in_a_session(displays).on_this_device();

    // Grabbed just inside its bottom right corner and carried past the area's, the monitor,
    // drawn smaller once the area refits at the lift, is kept inside the area by the rect it
    // is drawn at: flush with its right and bottom edges. Kept in by its larger lifted rect,
    // it would stop short of both.
    let desk = settings.editor.rect_of("display.arrangement");
    let lifted = settings.editor.rect_of("display.arrangement.DP-1");
    let fill = tile_fill(&settings.editor, "display.arrangement.DP-1");
    let from = lifted.max - vec2(4.0, 4.0);
    let to = desk.max + vec2(40.0, 40.0);
    settings.editor.down(from, PointerButton::Primary);
    settings.editor.hover(from + (to - from) / 2.0);
    settings.editor.hover(to);
    settings.run();
    let carried = drawn_tile(&settings.editor, fill);
    assert!(
        carried.width() < lifted.width() - 1.0,
        "the area refit for the carry: {carried:?} from {lifted:?}"
    );
    assert!(
        (carried.max - desk.max).abs().max_elem() < 1.5,
        "the monitor reaches the area's bottom right corner: {carried:?} in {desk:?}"
    );
    settings.editor.up(to, PointerButton::Primary);
    settings.run();
}

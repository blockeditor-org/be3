use super::*;
use be_sdk_beui::beui::{PointerButton, Pos2, Rect, Vec2, vec2};

/// Where `point` is in `rect`, in fractions of its size.
fn fraction(rect: Rect, point: Pos2) -> Vec2 {
    vec2(
        (point.x - rect.min.x) / rect.width(),
        (point.y - rect.min.y) / rect.height(),
    )
}

#[test]
fn a_carried_display_stays_under_the_pointer_where_it_was_grabbed() {
    let displays = laptop_beside(gaming_monitor());
    let mut settings = Harness::in_a_session(displays).on_this_device();

    // Grabbed just inside its bottom right corner, the monitor is drawn smaller once the
    // area fits every place at the lift: it is held there at that part of it, as drawn now.
    let lifted = settings.editor.rect_of("display.arrangement.DP-1");
    let fill = tile_fill(&settings.editor, "display.arrangement.DP-1");
    let from = lifted.max - vec2(4.0, 4.0);
    let grabbed = fraction(lifted, from);
    settings.editor.down(from, PointerButton::Primary);
    let lift = from + vec2(-10.0, 15.0);
    settings.editor.hover(lift);
    settings.run();
    // The refit lays the monitor out smaller after the point that lifted it: held still,
    // the pointer is on the same part of it as soon as it is drawn at that size.
    let refit = drawn_tile(&settings.editor, fill);
    let held = fraction(refit, lift);
    assert!(
        refit.width() < lifted.width() - 1.0 && (held - grabbed).length() < 0.02,
        "held still after the lift, the refit monitor is held at {grabbed:?}, not {held:?}: {refit:?} under {lift:?}"
    );
    let to = from + vec2(-20.0, 30.0);
    settings.editor.hover(to);
    settings.run();

    let carried = drawn_tile(&settings.editor, fill);
    assert!(
        carried.width() < lifted.width() - 1.0,
        "the area refit for the carry: {carried:?} from {lifted:?}"
    );
    let held = fraction(carried, to);
    assert!(
        carried.contains(to) && (held - grabbed).length() < 0.02,
        "the monitor is held at the part of it grabbed, {grabbed:?}, not {held:?}: {carried:?} under {to:?}"
    );
    settings.editor.up(to, PointerButton::Primary);
    settings.run();
}

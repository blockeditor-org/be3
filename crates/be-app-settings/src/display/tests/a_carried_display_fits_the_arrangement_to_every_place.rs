use super::*;
use crate::display::arrangement::{View, carry_places, tiles};
use be_sdk_beui::be_block::display_settings::Arrangement;
use be_sdk_beui::beui::{PointerButton, Pos2, Rect, pos2, vec2};

#[test]
fn a_carried_display_fits_the_arrangement_to_every_place() {
    let displays = laptop_beside(gaming_monitor());
    let mut settings = Harness::in_a_session(displays.clone()).on_this_device();

    let laid = tiles(&displays, &Arrangement::default());
    let rects: Vec<Rect> = laid.iter().map(|tile| tile.rect).collect();
    let fit = View::of(&rects);
    let carry = View::of(&[rects.clone(), carry_places(&laid, LAPTOP)].concat());
    let desk = settings.editor.rect_of("display.arrangement");
    let on_desk = |view: View, point: Pos2| desk.min + view.to_area(point).to_vec2();
    let (laptop, monitor) = (laid[0].rect, laid[1].rect);
    // The place below the monitor, at its left: its lower part is past the fit before the lift.
    let below = Rect::from_min_size(pos2(monitor.left(), monitor.bottom()), laptop.size());
    assert!(
        on_desk(fit, below.max).y > desk.max.y,
        "below the monitor is beyond the fit before the lift"
    );

    let from = on_desk(fit, laptop.center());
    settings.editor.down(from, PointerButton::Primary);
    settings.editor.hover(from + vec2(0.0, 8.0));
    settings.run();
    // At the lift the area fits every place the laptop can go, and holds that fit.
    let drawn = settings.editor.rect_of("display.arrangement.DP-1");
    assert!(
        (drawn.min - on_desk(carry, monitor.min)).length() < 1.0
            && (drawn.max - on_desk(carry, monitor.max)).length() < 1.0,
        "the monitor is drawn at the carry's fit: {drawn:?}"
    );
    let to = on_desk(carry, below.center());
    settings.editor.hover(from + (to - from) / 2.0);
    settings.editor.hover(to);
    settings.run();
    let carried = settings.editor.rect_of("display.arrangement.eDP-1");
    assert!(
        desk.contains_rect(carried),
        "the carried laptop is drawn whole, inside the area: {carried:?} in {desk:?}"
    );
    assert!(
        (carried.min - on_desk(carry, below.min)).length() < 1.0,
        "on the place below the monitor at once: {carried:?}"
    );
    assert_eq!(
        settings.editor.rect_of("display.arrangement.DP-1"),
        drawn,
        "and the area did not refit under it"
    );
    settings
        .editor
        .snapshot("a_carried_display_fits_the_arrangement_to_every_place");
    settings.editor.up(to, PointerButton::Primary);
    settings.run();

    let arrangement = settings.here().arrangement();
    let (laptop_at, monitor_at) = (
        arrangement.place(LAPTOP).expect("the laptop is placed"),
        arrangement.place(MONITOR).expect("the monitor is placed"),
    );
    assert_eq!(
        (laptop_at.0, laptop_at.1),
        (monitor_at.0, monitor_at.1 + monitor.height() as i32),
        "the laptop stands below the monitor, at its left"
    );
}

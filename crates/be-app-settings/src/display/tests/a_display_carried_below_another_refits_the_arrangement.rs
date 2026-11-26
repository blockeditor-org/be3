use super::*;
use crate::display::arrangement::{View, tiles};
use be_sdk_beui::be_block::display_settings::Arrangement;
use be_sdk_beui::beui::{PointerButton, Pos2, Rect, pos2};

#[test]
fn a_display_carried_below_another_refits_the_arrangement() {
    let displays = laptop_beside(gaming_monitor());
    let mut settings = Harness::in_a_session(displays.clone()).on_this_device();

    let laid = tiles(&displays, &Arrangement::default());
    let view = View::of(&laid.iter().map(|tile| tile.rect).collect::<Vec<_>>());
    let desk = settings.editor.rect_of("display.arrangement");
    let (laptop, monitor) = (laid[0].rect, laid[1].rect);
    // The place below the monitor, at its left: its lower part is past the area's fit.
    let below = Rect::from_min_size(pos2(monitor.left(), monitor.bottom()), laptop.size());
    let on_desk = |view: View, point: Pos2| desk.min + view.to_area(point).to_vec2();
    assert!(
        on_desk(view, below.max).y > desk.max.y,
        "below the monitor is beyond the fit at the lift"
    );
    let from = on_desk(view, laptop.center());
    let to = on_desk(view, below.center());
    assert!(to.y < desk.max.y, "though the pointer can reach it");
    settings.editor.down(from, PointerButton::Primary);
    settings.editor.hover(from + (to - from) / 2.0);
    settings.editor.hover(to);
    settings.run();

    // The area has refit to the arrangement that place makes: places are read from the
    // monitor's tile as it is drawn now.
    let shown = settings.editor.rect_of("display.arrangement.DP-1");
    let fit = shown.width() / monitor.width();
    let lifted = view.to_area(monitor.max).x - view.to_area(monitor.min).x;
    assert!(shown.width() < lifted - 1.0, "it zoomed out: {shown:?}");
    let refit = |point: Pos2| shown.min + (point - monitor.min) * fit;
    let held = refit(below.center());
    settings.editor.hover(held);
    settings.run();
    let carried = settings.editor.rect_of("display.arrangement.eDP-1");
    assert!(
        desk.contains_rect(carried),
        "the carried laptop is drawn whole, inside the area: {carried:?} in {desk:?}"
    );
    assert!(
        (carried.min - refit(below.min)).length() < 1.0,
        "on the place below the monitor: {carried:?}"
    );
    settings
        .editor
        .snapshot("a_display_carried_below_another_refits_the_arrangement");
    settings.editor.up(held, PointerButton::Primary);
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

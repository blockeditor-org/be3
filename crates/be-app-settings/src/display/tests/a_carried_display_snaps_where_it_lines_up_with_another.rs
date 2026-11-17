use super::*;
use crate::display::arrangement::{View, tiles};
use be_sdk_beui::be_block::display_settings::Arrangement;
use be_sdk_beui::beui::{PointerButton, pos2, vec2};

#[test]
fn a_carried_display_snaps_where_it_lines_up_with_another() {
    let displays = laptop_beside(gaming_monitor());
    let mut settings = Harness::in_a_session(displays.clone()).on_this_device();

    let laid = tiles(&displays, &Arrangement::default());
    let view = View::of(&laid.iter().map(|tile| tile.rect).collect::<Vec<_>>());
    let desk = settings.editor.rect_of("display.arrangement");
    let on_desk = |point: be_sdk_beui::beui::Pos2| desk.min + view.to_area(point).to_vec2();
    let (laptop, monitor) = (laid[0].rect, laid[1].rect);
    // Held just off the place below the laptop where their left edges line up.
    let grab = monitor.center() - monitor.min;
    let near = pos2(laptop.left(), laptop.bottom()) + grab;
    let from = on_desk(monitor.center());
    let to = on_desk(near) + vec2(4.0, 3.0);
    settings.editor.down(from, PointerButton::Primary);
    settings.editor.hover(from + (to - from) / 2.0);
    settings.editor.hover(to);
    settings
        .editor
        .snapshot("a_carried_display_snaps_where_it_lines_up_with_another");
    settings.editor.up(to, PointerButton::Primary);
    settings.run();

    let arrangement = settings.here().arrangement();
    assert_eq!(
        (
            arrangement.place(LAPTOP).map(|(x, _)| x),
            arrangement.place(MONITOR).map(|(x, _)| x)
        ),
        (Some(0), Some(0)),
        "the monitor lands below the laptop, their left edges lined up"
    );
    let (laptop_y, monitor_y) = (
        arrangement.place(LAPTOP).map(|(_, y)| y),
        arrangement.place(MONITOR).map(|(_, y)| y),
    );
    assert!(
        monitor_y > laptop_y,
        "below it: {monitor_y:?} under {laptop_y:?}"
    );
}

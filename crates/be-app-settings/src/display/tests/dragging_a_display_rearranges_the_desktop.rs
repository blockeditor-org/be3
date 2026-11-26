use super::*;
use crate::display::arrangement::{View, tiles};
use be_sdk_beui::be_block::display_settings::Arrangement;

#[test]
fn dragging_a_display_rearranges_the_desktop() {
    let displays = laptop_beside(gaming_monitor());
    let mut settings = Harness::in_a_session(displays.clone()).on_this_device();
    settings
        .editor
        .snapshot("dragging_a_display_rearranges_the_desktop");

    let laid = tiles(&displays, &Arrangement::default());
    let view = View::of(&laid.iter().map(|tile| tile.rect).collect::<Vec<_>>());
    let desk = settings.editor.rect_of("display.arrangement");
    let on_desk = |point: be_sdk_beui::beui::Pos2| desk.min + view.to_area(point).to_vec2();
    let (laptop, monitor) = (laid[0].rect, laid[1].rect);
    // Carried toward the laptop's left, to the area's edge: the place there is past the fit
    // at the lift, so the area refits to it, and the place is read again from the laptop's
    // tile as it is drawn now.
    let from = on_desk(monitor.center());
    let edge = be_sdk_beui::beui::pos2(desk.min.x + 4.0, on_desk(laptop.center()).y);
    settings.editor.down(from, be_sdk_beui::beui::PointerButton::Primary);
    settings.editor.hover(from + (edge - from) / 2.0);
    settings.editor.hover(edge);
    settings.run();
    let shown = settings.editor.rect_of("display.arrangement.eDP-1");
    let fit = shown.width() / laptop.width();
    let left = be_sdk_beui::beui::pos2(laptop.left() - monitor.width() / 2.0, laptop.center().y);
    let to = shown.min + (left - laptop.min) * fit;
    settings.editor.hover(to);
    settings.editor.up(to, be_sdk_beui::beui::PointerButton::Primary);
    settings.run();

    let arrangement = settings.here().arrangement();
    assert_eq!(
        arrangement.place(MONITOR).map(|(x, _)| x),
        Some(0),
        "the monitor now stands left of the laptop"
    );
    assert_eq!(
        arrangement.place(LAPTOP).map(|(x, _)| x),
        Some(2560),
        "and the laptop meets its right edge"
    );
    assert_eq!(arrangement.main, None, "the main display stays as it was");
    assert_eq!(
        settings.every_device().arrangement,
        None,
        "an arrangement is this device's alone"
    );
    settings
        .editor
        .snapshot("dragging_a_display_rearranges_the_desktop_after");
}

use super::*;
use crate::display::arrangement::{View, carry_places, tiles};
use be_sdk_beui::be_block::display_settings::Arrangement;

#[test]
fn dragging_a_display_rearranges_the_desktop() {
    let displays = laptop_beside(gaming_monitor());
    let mut settings = Harness::in_a_session(displays.clone()).on_this_device();
    settings
        .editor
        .snapshot("dragging_a_display_rearranges_the_desktop");

    let laid = tiles(&displays, &Arrangement::default());
    let rects: Vec<_> = laid.iter().map(|tile| tile.rect).collect();
    let view = View::of(&rects);
    // From the lift, the area fits every place the monitor can go.
    let carry = View::of(&[rects.clone(), carry_places(&laid, MONITOR)].concat());
    let desk = settings.editor.rect_of("display.arrangement");
    let on_desk =
        |view: View, point: be_sdk_beui::beui::Pos2| desk.min + view.to_area(point).to_vec2();
    let (laptop, monitor) = (laid[0].rect, laid[1].rect);
    let from = on_desk(view, monitor.center());
    let to = on_desk(carry, be_sdk_beui::beui::pos2(
        laptop.left() - monitor.width() / 2.0 + 40.0,
        laptop.center().y,
    ));
    let mid = from + (to - from) / 2.0;
    settings.editor.drag_along(&[from, mid, to]);
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
    let (laptop_y, monitor_y) = (
        arrangement.place(LAPTOP).map(|(_, y)| y),
        arrangement.place(MONITOR).map(|(_, y)| y),
    );
    assert_eq!(
        laptop_y.zip(monitor_y).map(|(laptop, monitor)| laptop - monitor),
        Some((1440 - 800) / 2),
        "at the monitor's middle"
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

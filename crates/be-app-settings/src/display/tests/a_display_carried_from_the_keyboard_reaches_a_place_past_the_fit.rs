use super::*;
use crate::display::arrangement::{View, tiles};
use be_sdk_beui::be_block::display_settings::Arrangement;
use be_sdk_beui::beui::{Key, Modifiers, Rect, pos2};

#[test]
fn a_display_carried_from_the_keyboard_reaches_a_place_past_the_fit() {
    let displays = laptop_beside(gaming_monitor());
    let mut settings = Harness::in_a_session(displays.clone()).on_this_device();
    let laid = tiles(&displays, &Arrangement::default());
    let (laptop, monitor) = (laid[0].rect, laid[1].rect);
    let fit = View::of(&[laptop, monitor]);
    let below = Rect::from_min_size(pos2(monitor.left(), monitor.bottom()), laptop.size());
    let desk = settings.editor.rect_of("display.arrangement");
    assert!(
        desk.min.y + fit.to_area(below.max).y > desk.max.y,
        "below the monitor is beyond the fit before the lift"
    );

    let tile = settings.editor.rect_of("display.arrangement.eDP-1").center();
    settings.editor.click(tile);
    settings.editor.key(Key::Space, Modifiers::NONE);
    settings.run();
    settings.editor.key(Key::ArrowDown, Modifiers::NONE);
    settings.run();
    let carried = settings.editor.rect_of("display.arrangement.eDP-1");
    assert!(
        desk.contains_rect(carried),
        "the laptop is carried in sight: {carried:?} in {desk:?}"
    );
    settings.editor.key(Key::Enter, Modifiers::NONE);
    settings.run();

    let arrangement = settings.here().arrangement();
    let (laptop_at, monitor_at) = (
        arrangement.place(LAPTOP).expect("the laptop is placed"),
        arrangement.place(MONITOR).expect("the monitor is placed"),
    );
    assert_eq!(
        (laptop_at.0, laptop_at.1),
        (monitor_at.0, monitor_at.1 + monitor.height() as i32),
        "the laptop stands below the monitor, at its left: {laptop_at:?} by {monitor_at:?}"
    );
}

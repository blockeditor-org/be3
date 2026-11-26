use super::*;
use crate::display::arrangement::tiles;
use be_sdk_beui::be_block::display_settings::Arrangement;
use be_sdk_beui::beui::{Key, Modifiers};

#[test]
fn a_focused_display_steps_to_the_next_place_from_the_keyboard() {
    let displays = laptop_beside(gaming_monitor());
    let mut settings = Harness::in_a_session(displays.clone()).on_this_device();
    let laid = tiles(&displays, &Arrangement::default());
    let (laptop, monitor) = (laid[0].rect, laid[1].rect);
    assert!(
        monitor.top() < laptop.top(),
        "the taller monitor starts with its bottom lined up with the laptop's"
    );

    let tile = settings.editor.rect_of("display.arrangement.DP-1").center();
    settings.editor.click(tile);
    let step = Modifiers {
        ctrl: true,
        shift: true,
        ..Modifiers::NONE
    };
    settings.editor.key(Key::ArrowDown, step);
    settings.run();

    let arrangement = settings.here().arrangement();
    let (laptop_at, monitor_at) = (
        arrangement.place(LAPTOP).expect("the laptop is placed"),
        arrangement.place(MONITOR).expect("the monitor is placed"),
    );
    assert_eq!(
        monitor_at.0,
        laptop_at.0 + laptop.width() as i32,
        "the monitor stays right of the laptop"
    );
    assert!(
        monitor_at.1 - laptop_at.1 > (monitor.top() - laptop.top()) as i32,
        "and steps down along its edge: {monitor_at:?} beside {laptop_at:?}"
    );
}

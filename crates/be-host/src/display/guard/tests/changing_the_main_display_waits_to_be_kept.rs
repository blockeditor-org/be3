use super::*;
use be_block::display_settings::{Arrangement, ScreenPlace};

#[test]
fn changing_the_main_display_waits_to_be_kept() {
    let mut guard = showing(office_monitor(), saved(Some(slow())));
    let place = |x: i32| {
        DisplaySettings::set_arrangement(Some(Arrangement {
            main: None,
            places: vec![ScreenPlace {
                id: MONITOR.to_owned(),
                x,
                y: 0,
            }],
        }))
    };
    let moved = edited(&saved(Some(slow())), &[place(1920)]);
    assert!(
        guard.take(moved.clone()),
        "moving a display in the arrangement applies at once"
    );

    let main = edited(
        &moved,
        &[DisplaySettings::set_arrangement(Some(Arrangement {
            main: Some(MONITOR.to_owned()),
            ..moved.arrangement()
        }))],
    );
    assert!(!guard.take(main), "a new main display waits to be kept");
    assert_eq!(
        guard.revert(),
        vec![Change::Arrangement(moved.arrangement.clone())]
    );
    assert_eq!(guard.applied(), &moved);
    assert_eq!(guard.keep(), Vec::new());
}

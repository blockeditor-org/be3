use super::*;

#[test]
fn a_synced_mode_is_asked_about_the_first_time_this_device_shows_it() {
    let mut guard = Guard::default();
    guard.take(saved(Some(small())));
    guard.set_screens(vec![gaming_monitor()]);
    assert!(
        guard.asking(),
        "a mode another device chose for this monitor asks to be kept here"
    );

    assert_eq!(guard.keep(), Vec::new(), "the mode is already saved");
    assert!(!guard.asking());
    assert_eq!(guard.tried().get(MONITOR), Some(&small()));
}

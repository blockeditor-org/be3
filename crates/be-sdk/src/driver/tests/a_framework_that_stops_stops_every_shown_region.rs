use super::*;

#[test]
fn a_framework_that_stops_stops_every_shown_region() {
    let mut driver = driven(Plain {
        stopped: Some(Some(Stopped::Closed)),
        ..Plain::default()
    });
    let frames = driver.frame(&[shown(1), shown(2)]);
    assert_eq!(frames.len(), 2);
    assert!(
        frames
            .iter()
            .all(|frame| frame.stopped == Some(Stopped::Closed))
    );
}

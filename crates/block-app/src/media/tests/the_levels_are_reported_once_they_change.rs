use super::*;

#[test]
fn the_levels_are_reported_once_they_change() {
    let (mut media, _recorder, events) = recorded();
    assert_eq!(media.frame(), None, "nothing is reported before a backend speaks");

    let output = Some(MediaLevel {
        level: 0.4,
        muted: false,
    });
    events
        .send(MediaEvent::Audio {
            output,
            input: None,
        })
        .unwrap();
    events.send(MediaEvent::Brightness(0.25)).unwrap();
    assert_eq!(
        media.frame(),
        Some(MediaLevels {
            output,
            input: None,
            brightness: Some(0.25),
        })
    );

    events
        .send(MediaEvent::Audio {
            output,
            input: None,
        })
        .unwrap();
    assert_eq!(media.frame(), None, "levels that did not change are not sent again");
}

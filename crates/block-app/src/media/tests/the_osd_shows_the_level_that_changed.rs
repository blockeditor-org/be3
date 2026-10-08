use super::*;

#[test]
fn the_osd_shows_the_level_that_changed() {
    let mut harness = Harness::new();
    harness.apply(MediaEvent::Audio(AudioLevels {
        output: Some(Level {
            volume: 0.4,
            muted: false,
        }),
        input: Some(Level {
            volume: 0.8,
            muted: true,
        }),
    }));
    assert_eq!(harness.osd(), None, "nothing shows before a key is pressed");

    harness.press(Key::VolumeUp);
    assert_eq!(harness.shows(), 1);
    assert_eq!(
        harness.osd(),
        Some(OsdLevel {
            glyph: ICON_VOLUME_DOWN.to_owned(),
            label: "Volume".to_owned(),
            level: 0.4,
            muted: false,
        })
    );
    harness.apply(MediaEvent::Audio(AudioLevels {
        output: Some(Level {
            volume: 0.45,
            muted: false,
        }),
        input: None,
    }));
    assert_eq!(
        harness.osd().map(|osd| osd.level),
        Some(0.45),
        "the level follows the volume the server reports"
    );

    harness.apply(MediaEvent::Audio(AudioLevels {
        output: None,
        input: Some(Level {
            volume: 0.8,
            muted: true,
        }),
    }));
    harness.press(Key::MicMute);
    assert_eq!(harness.shows(), 2);
    assert_eq!(
        harness.osd(),
        Some(OsdLevel {
            glyph: ICON_MIC_OFF.to_owned(),
            label: "Microphone".to_owned(),
            level: 0.8,
            muted: true,
        })
    );

    harness.press(Key::BrightnessDown);
    assert_eq!(
        harness.osd(),
        None,
        "the brightness shows once the backlight reports it"
    );
    harness.apply(MediaEvent::Brightness(0.3));
    assert_eq!(
        harness.osd(),
        Some(OsdLevel {
            glyph: ICON_BRIGHTNESS_LOW.to_owned(),
            label: "Brightness".to_owned(),
            level: 0.3,
            muted: false,
        })
    );

    let shows = harness.shows();
    harness.press(Key::MediaNext);
    assert_eq!(harness.shows(), shows, "a player's key shows no level");
}

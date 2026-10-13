use super::*;

#[test]
fn media_keysyms_map_to_the_media_keys() {
    let media = [
        (keysyms::KEY_XF86AudioRaiseVolume, Key::VolumeUp),
        (keysyms::KEY_XF86AudioLowerVolume, Key::VolumeDown),
        (keysyms::KEY_XF86AudioMute, Key::VolumeMute),
        (keysyms::KEY_XF86AudioMicMute, Key::MicMute),
        (keysyms::KEY_XF86MonBrightnessUp, Key::BrightnessUp),
        (keysyms::KEY_XF86MonBrightnessDown, Key::BrightnessDown),
        (keysyms::KEY_XF86AudioPlay, Key::MediaPlayPause),
        (keysyms::KEY_XF86AudioPause, Key::MediaPlayPause),
        (keysyms::KEY_XF86AudioNext, Key::MediaNext),
        (keysyms::KEY_XF86AudioPrev, Key::MediaPrevious),
        (keysyms::KEY_XF86AudioStop, Key::MediaStop),
    ];
    for (keysym, expected) in media {
        assert_eq!(key(keysym), Some(expected));
        assert!(expected.is_media());
    }
}

use super::*;

#[test]
fn every_key_beui_names_has_a_way_to_be_injected() {
    for key in Key::ALL {
        let Some(evdev) = key.evdev() else {
            continue;
        };
        if matches!(key, Key::MicMute | Key::BrightnessUp | Key::BrightnessDown) {
            continue;
        }
        assert!(
            scan_of_evdev(evdev).is_some() || virtual_key_of_evdev(evdev).is_some(),
            "{key:?}"
        );
    }
}

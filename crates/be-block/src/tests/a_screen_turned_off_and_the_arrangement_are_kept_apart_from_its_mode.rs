use super::*;

use crate::display_settings::{Arrangement, ScreenPlace, ScreenUse};

#[test]
fn a_screen_turned_off_and_the_arrangement_are_kept_apart_from_its_mode() {
    let laptop = "BOE|0x0A1B|0";
    let monitor = "DEL|U2723QE|1234";
    let settings = DisplaySettingsContent::default();
    assert_eq!(settings.root().used(monitor), ScreenUse::Desktop);
    assert_eq!(settings.root().main_screen(), None);

    let settings = edited(
        &settings,
        [settings.root().set_used(
            laptop,
            Some(ScreenUse::Mirror {
                of: monitor.to_owned(),
            }),
        )],
    );
    let arrangement = Arrangement {
        main: Some(monitor.to_owned()),
        places: vec![
            ScreenPlace {
                id: monitor.to_owned(),
                x: 0,
                y: 0,
            },
            ScreenPlace {
                id: laptop.to_owned(),
                x: 2560,
                y: 360,
            },
        ],
    };
    let settings = edited(
        &settings,
        [DisplaySettings::set_arrangement(Some(arrangement.clone()))],
    );
    assert_eq!(
        settings.root().used(laptop),
        ScreenUse::Mirror {
            of: monitor.to_owned()
        }
    );
    assert_eq!(settings.root().used(monitor), ScreenUse::Desktop);
    assert_eq!(settings.root().main_screen().as_deref(), Some(monitor));
    assert_eq!(settings.root().arrangement().place(laptop), Some((2560, 360)));
    assert_eq!(settings.root().mode(laptop), None, "a use is set apart from a mode");

    let settings = edited(&settings, [settings.root().set_used(laptop, None)]);
    assert!(
        settings.root().screens.is_empty(),
        "a screen back on the desktop is forgotten"
    );
}

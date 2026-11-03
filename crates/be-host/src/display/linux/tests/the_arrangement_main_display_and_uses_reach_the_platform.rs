use be_block::DisplaySettingsContent;
use be_block::display_settings::{Arrangement, ScreenPlace};

use super::*;

const LAPTOP: &str = "BOE|NE135A1M-NY1|0";
const MONITOR: &str = "DEL|U2723QE|1234";
const PROJECTOR: &str = "HDMI-A-1";

#[test]
fn the_arrangement_main_display_and_uses_reach_the_platform() {
    let mut settings = DisplaySettingsContent::default();
    let root = settings.root();
    settings.apply(&root.set_used(
        PROJECTOR,
        Some(ScreenUse::Mirror {
            of: LAPTOP.to_owned(),
        }),
    ));
    let root = settings.root();
    settings.apply(&root.set_used(MONITOR, Some(ScreenUse::Off)));
    settings.apply(&DisplaySettings::set_arrangement(Some(Arrangement {
        main: Some(LAPTOP.to_owned()),
        places: vec![ScreenPlace {
            id: LAPTOP.to_owned(),
            x: 2560,
            y: 640,
        }],
    })));

    let config = config(&settings.root());
    let mut uses = config.uses.clone();
    uses.sort_by(|a, b| a.0.cmp(&b.0));
    assert_eq!(
        uses,
        [
            (MONITOR.to_owned(), DisplayUse::Off),
            (PROJECTOR.to_owned(), DisplayUse::Mirror(LAPTOP.to_owned())),
        ]
    );
    assert_eq!(config.main.as_deref(), Some(LAPTOP));
    assert_eq!(
        config.places,
        [(LAPTOP.to_owned(), beui::pos2(2560.0, 640.0))]
    );
}

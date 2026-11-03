use be_block::{DisplaySettingsContent, Layered};

use super::*;

const LAPTOP: &str = "BOE|NE135A1M-NY1|0";
const MONITOR: &str = "DEL|U2723QE|1234";
const PROJECTOR: &str = "HDMI-A-1";

#[test]
fn each_display_is_configured_at_the_scale_chosen_for_it() {
    let mut everywhere = DisplaySettingsContent::default();
    let settings = everywhere.root();
    everywhere.apply(&settings.set_screen_scale(MONITOR, Some(150)));
    let settings = everywhere.root();
    everywhere.apply(&settings.set_screen_scale(LAPTOP, Some(175)));
    let mut here = DisplaySettingsContent::default();
    let settings = here.root();
    here.apply(&settings.set_screen_scale(LAPTOP, Some(225)));
    let settings = here.root();
    here.apply(&settings.set_rotation(PROJECTOR, Some(be_block::display_settings::Rotation::Clockwise90)));

    let config = config(&here.root().over(&everywhere.root()));
    let mut scales = config.scales.clone();
    scales.sort_by(|a, b| a.0.cmp(&b.0));
    assert_eq!(
        scales,
        [(LAPTOP.to_owned(), 2.25), (MONITOR.to_owned(), 1.5)],
        "this device's scale wins over every device's, and a display with none chosen is left to its default"
    );
}

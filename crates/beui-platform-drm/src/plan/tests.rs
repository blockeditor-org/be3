use super::*;

mod a_display_mirrors_only_a_display_on_the_desktop;
mod the_main_display_comes_first_and_those_off_or_mirroring_last;
mod turning_every_display_off_keeps_the_main_one_lit;

const LAPTOP: &str = "BOE|0x0A1B|0";
const LEFT: &str = "DEL|U2723QE|1";
const RIGHT: &str = "DEL|U2723QE|2";

fn config(uses: &[(&str, DisplayUse)], main: Option<&str>) -> DisplayConfig {
    DisplayConfig {
        uses: uses
            .iter()
            .map(|(id, used)| ((*id).to_owned(), used.clone()))
            .collect(),
        main: main.map(str::to_owned),
        ..DisplayConfig::default()
    }
}

use super::*;

mod a_change_no_one_answers_expires_once_the_time_to_answer_is_up;
mod a_change_to_a_monitor_that_is_not_connected_is_not_asked_about;
mod a_changed_mode_waits_to_be_kept;
mod a_kept_default_is_not_asked_about_again;
mod a_monitor_whose_default_is_its_preferred_mode_is_not_asked_about;
mod an_untried_fastest_default_falls_back_to_the_preferred_mode;
mod another_change_while_asking_asks_again_with_the_full_time;
mod reverting_a_change_leaves_monitors_that_are_not_connected_alone;
mod reverting_a_change_restores_the_mode_before_it;

const MONITOR: &str = "DEL|U2723QE|1234";

fn mode(width: u32, height: u32, refresh_millihertz: u32) -> DisplayMode {
    DisplayMode {
        width,
        height,
        refresh_millihertz,
    }
}

fn slow() -> DisplayMode {
    mode(2560, 1440, 59_951)
}

fn fast() -> DisplayMode {
    mode(2560, 1440, 143_998)
}

fn small() -> DisplayMode {
    mode(1920, 1080, 60_000)
}

fn gaming_monitor() -> Screen {
    Screen {
        id: MONITOR.to_owned(),
        modes: vec![fast(), slow(), small()],
        default: fast(),
        preferred: slow(),
    }
}

fn office_monitor() -> Screen {
    Screen {
        id: MONITOR.to_owned(),
        modes: vec![slow(), small()],
        default: slow(),
        preferred: slow(),
    }
}

fn saved(mode: Option<DisplayMode>) -> DisplaySettings {
    edited(
        &DisplaySettings::default(),
        &[DisplaySettings::set_mode(MONITOR, mode)],
    )
}

fn showing(screen: Screen, settings: DisplaySettings) -> Guard {
    let mut guard = Guard::default();
    guard.take(settings);
    guard.set_screens(vec![screen]);
    guard
}

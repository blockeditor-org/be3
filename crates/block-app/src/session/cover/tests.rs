use super::*;

mod a_desktop_that_does_not_draw_in_time_gets_the_built_in_lock_screen;
mod a_desktop_that_draws_its_lock_screen_is_shown_and_given_the_password;
mod a_desktop_that_stops_answering_gets_the_built_in_lock_screen;
mod a_desktop_that_stops_gets_the_built_in_lock_screen_until_the_next_lock;
mod a_desktop_without_a_lock_screen_gets_the_built_in_one_at_once;

const DRAWN: PluginLock = PluginLock {
    offered: true,
    failed: false,
    drawn: true,
    unanswered: None,
};

const STARTING: PluginLock = PluginLock {
    drawn: false,
    ..DRAWN
};

fn millis(millis: u64) -> Duration {
    Duration::from_millis(millis)
}

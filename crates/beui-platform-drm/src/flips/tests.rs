use super::*;

mod a_flip_from_before_a_screen_was_drawn_again_is_not_reported;

fn flip(screen: &str, seconds: u64) -> Flip {
    Flip {
        screen: screen.to_owned(),
        time: Duration::from_secs(seconds),
        refresh: Duration::from_millis(16),
        sequence: seconds,
        hardware_clock: true,
    }
}

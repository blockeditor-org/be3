use super::*;

mod a_chord_ends_at_a_key_pressed_while_others_are_held;
mod a_wheel_notch_ends_a_chord_with_what_is_held;
mod keys_held_when_it_starts_pass_until_released;
mod keys_still_held_after_the_chord_are_kept_until_released;
mod the_first_release_ends_a_chord_of_what_was_pressed;
mod the_primary_button_the_session_keys_and_the_lock_screen_pass;

fn key(name: &str) -> Source {
    Source::named(name).expect("a known name")
}

fn press(name: &str) -> Input {
    Input::Press(key(name))
}

fn release(name: &str) -> Input {
    Input::Release(key(name))
}

fn caught(capture: &mut Capture, inputs: &[Input]) -> Vec<Input> {
    inputs
        .iter()
        .flat_map(|input| capture.input(*input))
        .collect()
}

fn started() -> Capture {
    let mut capture = Capture::default();
    capture.start();
    capture
}

fn chord(held: &[&str], trigger: &str) -> Chord {
    Chord {
        held: held.iter().map(|name| key(name)).collect(),
        trigger: Caught::Source(key(trigger)),
    }
}

use super::*;

mod a_held_modifier_is_lifted_for_a_tap_and_put_back_after_it;
mod a_remap_only_applies_in_the_window_it_names;
mod a_remapped_button_holds_its_key_until_it_is_let_go;
mod a_sequence_plays_its_steps_on_the_clock;
mod a_wheel_notch_taps_a_key_for_as_long_as_the_rule_holds_it;
mod an_autohotkey_script_reads_as_remaps;
mod held_conditions_pick_the_rule;
mod kept_input_still_reaches_the_window;
mod key_names_read_back_as_they_are_written;
mod text_that_names_no_key_says_which_rule;
mod wheel_turns_no_rule_takes_pass_through;

const NOW: u64 = 1_000_000;
const MS: u64 = 1000;

fn key(name: &str) -> Source {
    Source::named(name).expect("a known name")
}

fn press(name: &str) -> Input {
    Input::Press(key(name))
}

fn release(name: &str) -> Input {
    Input::Release(key(name))
}

fn engine(text: &str) -> Engine {
    Engine::new(text::parse(text).expect("the rules parse"), None, &[], &[])
}

fn window(exe: &str) -> Window {
    Window {
        app_id: "steam_app_1".to_owned(),
        title: "A game".to_owned(),
        exe: exe.to_owned(),
    }
}

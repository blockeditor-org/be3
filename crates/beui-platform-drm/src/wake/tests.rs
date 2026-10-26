use super::*;

mod a_click_that_wakes_the_screens_reaches_nothing;
mod a_key_held_across_the_wake_leaks_no_unmatched_release;
mod input_in_the_grace_period_is_swallowed_and_input_after_it_is_delivered;

const MS: u64 = 1000;

fn swallowed(woke: bool) -> Pass {
    Pass {
        deliver: false,
        woke,
    }
}

fn delivered(woke: bool) -> Pass {
    Pass {
        deliver: true,
        woke,
    }
}

fn blanked() -> WakeGate {
    let mut gate = WakeGate::default();
    gate.set_blanked(true);
    gate
}

use super::*;

mod a_failed_power_call_ends_the_ending_as_cancelling_does;
mod a_program_that_never_closes_holds_the_session_until_it_is_ended_now;
mod cancelling_mid_way_leaves_the_programs_still_open;
mod closing_the_desktop_waits_for_its_programs;
mod logging_out_waits_for_programs_to_close_then_exits;
mod restarting_and_powering_off_ask_logind_once_the_programs_have_gone;
mod suspending_asks_logind_at_once_and_only_when_it_can;

const EVERYTHING: LogindAbilities = LogindAbilities {
    suspend: true,
    reboot: true,
    power_off: true,
};

#[derive(Default)]
struct Recorder {
    calls: Vec<LogindCall>,
    windows: usize,
    closes: usize,
    exits: usize,
    problems: Vec<String>,
}

impl SessionControl for Recorder {
    fn call(&mut self, call: LogindCall) {
        self.calls.push(call);
    }

    fn windows(&self) -> usize {
        self.windows
    }

    fn close_windows(&mut self) {
        self.closes += 1;
    }

    fn exit(&mut self) {
        self.exits += 1;
    }

    fn report(&mut self, problem: String) {
        self.problems.push(problem);
    }
}

fn power() -> Power {
    let mut power = Power::default();
    power.set_abilities(EVERYTHING);
    power.log_in();
    power
}

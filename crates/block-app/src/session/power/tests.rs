use super::*;

mod a_failed_power_off_is_reported_and_leaves_the_session_running;
mod logging_out_waits_for_programs_to_close_then_exits;
mod restarting_and_powering_off_ask_logind_once_the_programs_have_gone;
mod suspending_asks_logind_at_once_and_only_when_it_can;
mod the_session_ends_after_the_grace_period_even_if_a_program_stays;

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
    power
}

fn seconds(seconds: u64) -> Duration {
    Duration::from_secs(seconds)
}

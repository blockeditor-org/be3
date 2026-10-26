use super::*;

mod a_failed_frame_sets_the_display_up_again_once_then_reports_once;
mod a_hotplug_retries_a_stalled_display;
mod failures_while_away_change_nothing;
mod other_displays_keep_their_own_streak;
mod taking_the_seat_back_rebuilds_every_display_from_scratch;

#[derive(Clone, Debug, Eq, PartialEq)]
enum Call {
    Suspend,
    Release(Option<u32>),
    TakeBack,
    Rescan,
    Stall(u32),
    Report,
}

#[derive(Default)]
struct FakeCard {
    calls: Vec<Call>,
}

impl FakeCard {
    fn take(&mut self) -> Vec<Call> {
        std::mem::take(&mut self.calls)
    }
}

impl Card for FakeCard {
    type Output = u32;

    fn suspend(&mut self) {
        self.calls.push(Call::Suspend);
    }

    fn release(&mut self, output: Option<u32>) {
        self.calls.push(Call::Release(output));
    }

    fn take_back(&mut self) {
        self.calls.push(Call::TakeBack);
    }

    fn rescan(&mut self) {
        self.calls.push(Call::Rescan);
    }

    fn stall(&mut self, output: u32) {
        self.calls.push(Call::Stall(output));
    }

    fn report(&mut self, _problem: String) {
        self.calls.push(Call::Report);
    }
}

const FLIP_FAILED: &str = "DP-1 could not show a frame: Page flip commit failed (Invalid argument)";

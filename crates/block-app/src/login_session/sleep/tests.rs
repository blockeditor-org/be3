use super::*;

mod suspending_locks_first_and_lets_the_computer_sleep_once_the_lock_is_shown;

#[derive(Debug, Default, PartialEq, Eq)]
struct Recorder {
    taken: usize,
    released: usize,
}

impl Inhibit for Recorder {
    fn take(&mut self) {
        self.taken += 1;
    }

    fn release(&mut self) {
        self.released += 1;
    }
}

use super::*;

mod a_password_that_cannot_be_checked_leaves_the_screen_locked;
mod a_wrong_password_leaves_the_screen_locked_and_says_so;
mod an_answer_to_an_earlier_attempt_unlocks_nothing;
mod each_trigger_locks_the_screen;
mod repeated_wrong_passwords_make_the_next_attempt_wait;
mod the_right_password_unlocks_the_screen;

#[derive(Default)]
struct Recorder {
    attempts: Vec<(u64, String)>,
}

impl Authenticate for Recorder {
    fn start(&mut self, attempt: u64, password: Password) {
        self.attempts.push((attempt, password.as_str().to_owned()));
    }
}

impl Recorder {
    fn last(&self) -> u64 {
        self.attempts.last().expect("an attempt was started").0
    }
}

fn locked() -> Lock {
    let mut lock = Lock::default();
    lock.lock(Trigger::Shortcut);
    lock
}

fn password(typed: &str) -> Password {
    Password::new(typed.to_owned())
}

fn seconds(seconds: u64) -> Duration {
    Duration::from_secs(seconds)
}

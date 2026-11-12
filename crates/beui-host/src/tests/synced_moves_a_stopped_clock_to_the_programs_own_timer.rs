use std::time::{Duration, Instant};

use super::*;

/// How long the program waits, quiet, before it saves: a timer of its own on the frame clock.
const QUIET: Duration = Duration::from_secs(30);

/// A program that edited and owes a save, which it makes once its frame clock has run `QUIET`
/// past the edit, as be-host's type modules seal after a quiet spell.
#[derive(Default)]
struct Owing {
    due: Option<Instant>,
    saved: bool,
}

impl Program for Owing {
    fn update(&mut self, context: &Context, _rect: Rect) {
        let due = *self.due.get_or_insert(context.now() + QUIET);
        self.saved |= context.now() >= due;
    }

    fn out_of_step(&self) -> Option<Vec<String>> {
        Some(match self.saved {
            true => Vec::new(),
            false => vec!["owes a save".to_owned()],
        })
    }

    fn in_step_due(&self) -> Option<Instant> {
        self.due.filter(|_| !self.saved)
    }
}

#[test]
fn synced_moves_a_stopped_clock_to_the_programs_own_timer() {
    let _root = crate::reactive::RootScope::new();
    let mut driven = headless(Owing::default());
    driven.ask(&["clock", "stop"]).expect("the clock stops");
    driven
        .ask(&["synced"])
        .expect("synced answers once the program's timer has come due");
    assert!(
        driven.program().saved,
        "the stopped clock was moved to the program's timer, so it saved"
    );
}

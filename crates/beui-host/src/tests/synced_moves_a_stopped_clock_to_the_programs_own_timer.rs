use std::time::{Duration, Instant};

use super::*;

/// How long the program waits, quiet, before it saves: a timer of its own on the frame clock.
const QUIET: Duration = Duration::from_secs(30);

/// A program that edited and owes a save, which it makes once its frame clock has run `QUIET`
/// past the edit, as be-host's type modules seal after a quiet spell. For its first `working`
/// frames it is busy, as with a request in flight that could still make progress.
#[derive(Default)]
struct Owing {
    working: u32,
    due: Option<Instant>,
    saved: bool,
    /// The frame clock at each frame it was busy: a frame's step at most, never the timer.
    busy_at: Vec<Instant>,
}

impl Program for Owing {
    fn update(&mut self, context: &Context, _rect: Rect) {
        let due = *self.due.get_or_insert(context.now() + QUIET);
        if self.working > 0 {
            self.working -= 1;
            self.busy_at.push(context.now());
            context.request_repaint();
        }
        self.saved |= context.now() >= due;
    }

    fn busy(&self) -> Vec<String> {
        match self.working {
            0 => Vec::new(),
            _ => vec!["a request is in flight".to_owned()],
        }
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
    let mut driven = headless(Owing {
        working: 3,
        ..Owing::default()
    });
    driven.ask(&["clock", "stop"]).expect("the clock stops");
    driven
        .ask(&["synced"])
        .expect("synced answers once the program's timer has come due");
    let program = driven.program();
    assert!(
        program.saved,
        "the stopped clock was moved to the program's timer, so it saved"
    );
    assert_eq!(program.busy_at.len(), 3, "the program was busy for 3 frames");
    let due = program.due.expect("the program set its timer");
    assert!(
        program.busy_at.iter().all(|at| *at + QUIET / 2 < due),
        "the clock was not moved to the timer while the program was busy: {:?}",
        program.busy_at
    );
}

use super::*;

#[derive(Default)]
struct Apps {
    pumps: u32,
    relaid_at: Option<u32>,
    answered_at: Option<u32>,
    requests: u32,
    /// Real time, which waiting moves on.
    clock: Duration,
}

impl Settling for Apps {
    fn pump(&mut self) -> bool {
        self.pumps += 1;
        self.relaid_at == Some(self.pumps)
    }

    fn request_frames(&mut self) {
        self.requests += 1;
    }

    fn outstanding(&self) -> bool {
        self.answered_at.is_none_or(|at| self.pumps < at)
    }

    fn wait(&mut self, _rung: u64, deadline: Duration) -> bool {
        self.clock += Duration::from_millis(1);
        self.clock < deadline
    }

    fn rung(&self) -> u64 {
        0
    }

    fn now(&self) -> Duration {
        self.clock
    }
}

#[test]
fn settling_ends_at_its_deadline() {
    let soon = Duration::from_secs(60);
    let mut apps = Apps {
        answered_at: Some(3),
        ..Apps::default()
    };
    assert_eq!(settle(&mut apps, soon), Settled::Answered);
    assert_eq!((apps.pumps, apps.requests), (3, 3));

    let mut apps = Apps {
        relaid_at: Some(2),
        ..Apps::default()
    };
    assert_eq!(settle(&mut apps, soon), Settled::Relaid);
    assert_eq!(
        apps.requests, 1,
        "a relaid host lays out before asking again"
    );

    let mut clock = FrameClock::default();
    let mut apps = Apps::default();
    assert!(!clock.settle(&mut apps, Duration::from_millis(3)));
    assert_eq!(
        clock.take_waited(),
        (Duration::from_millis(3), 1),
        "the frame ran out of time, having waited until its deadline"
    );
}

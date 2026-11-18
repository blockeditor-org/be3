use super::*;

#[derive(Default)]
struct Apps {
    pumps: u32,
    relaid_at: Option<u32>,
    answered_at: Option<u32>,
    requests: u32,
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

    fn wait(&mut self, _rung: u64, deadline: Instant) -> bool {
        Instant::now() < deadline
    }

    fn rung(&self) -> u64 {
        0
    }
}

#[test]
fn settling_ends_at_its_deadline() {
    let soon = Instant::now() + Duration::from_secs(60);
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
    assert!(!clock.settle(&mut apps, Instant::now()));
    assert_eq!(clock.take_waited().1, 1, "the frame ran out of time");
}

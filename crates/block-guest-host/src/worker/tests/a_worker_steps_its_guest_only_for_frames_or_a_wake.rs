use super::*;

#[derive(Default)]
struct Counted {
    events: Vec<Event<std::convert::Infallible>>,
    stepped: usize,
    woken: bool,
}

impl Channel for Counted {
    type Presented = std::convert::Infallible;

    fn step(&mut self, _frames: Vec<Vec<u8>>) -> bool {
        self.stepped += 1;
        self.events.push(Event::Stepped(Produced::sent(
            Vec::new(),
            StepTime::default(),
        )));
        true
    }

    fn receive(&mut self) -> Received<Self::Presented> {
        match self.events.pop() {
            Some(event) => Received::Event(event),
            None => Received::Empty,
        }
    }

    fn take_woken(&mut self) -> bool {
        std::mem::take(&mut self.woken)
    }
}

#[test]
fn a_worker_steps_its_guest_only_for_frames_or_a_wake() {
    let mut worker = Worker::new(
        Counted {
            events: vec![Event::Ready(Produced::sent(
                Vec::new(),
                StepTime::default(),
            ))],
            ..Counted::default()
        },
        Arc::default(),
    );
    for _ in 0..3 {
        assert_eq!(worker.poll(), None);
    }
    assert_eq!(worker.channel.stepped, 0, "an idle guest is left alone");

    assert_eq!(worker.queue(vec![b"frame".to_vec()]), None);
    for _ in 0..3 {
        assert_eq!(worker.poll(), None);
    }
    assert_eq!(
        worker.channel.stepped, 1,
        "a frame is delivered in one step"
    );

    worker.channel.woken = true;
    for _ in 0..3 {
        assert_eq!(worker.poll(), None);
    }
    assert_eq!(
        worker.channel.stepped, 2,
        "a guest that woke is stepped once"
    );
    assert!(worker.settled());
}

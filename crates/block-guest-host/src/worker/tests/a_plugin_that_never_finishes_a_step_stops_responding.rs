use super::*;

#[derive(Default)]
struct Stuck {
    events: Vec<Event<std::convert::Infallible>>,
    stepped: usize,
}

impl Channel for Stuck {
    type Presented = std::convert::Infallible;

    fn step(&mut self, _frames: Vec<Vec<u8>>) -> bool {
        self.stepped += 1;
        true
    }

    fn take_woken(&mut self) -> bool {
        true
    }

    fn receive(&mut self) -> Received<Self::Presented> {
        match self.events.pop() {
            Some(event) => Received::Event(event),
            None => Received::Empty,
        }
    }
}

fn produced() -> Produced<std::convert::Infallible> {
    Produced {
        outbound: Vec::new(),
        presented: Vec::new(),
        painted: false,
        took: StepTime::default(),
    }
}

#[test]
fn a_plugin_that_never_finishes_a_step_stops_responding() {
    let mut worker = Worker::new(
        Stuck {
            events: vec![Event::Ready(produced())],
            stepped: 0,
        },
        Arc::default(),
    );
    assert_eq!(worker.poll(), None);
    assert_eq!(worker.channel.stepped, 1, "the first step is sent");
    let start = Duration::from_secs(100);
    assert_eq!(worker.hung(start), Ok(Some(STEP_TIMEOUT)));

    worker.channel.events.push(Event::Stepped(produced()));
    assert_eq!(worker.poll(), None);
    assert_eq!(
        worker.channel.stepped, 2,
        "a finished step lets the next go"
    );
    let later = start + STEP_TIMEOUT - Duration::from_secs(1);
    assert_eq!(
        worker.hung(later),
        Ok(Some(STEP_TIMEOUT)),
        "a new step starts its own time"
    );

    assert_eq!(
        worker.hung(later + STEP_TIMEOUT / 2),
        Ok(Some(STEP_TIMEOUT / 2)),
        "a step still within its time is waited for"
    );
    assert_eq!(
        worker.hung(later + STEP_TIMEOUT),
        Err(NOT_RESPONDING.to_owned()),
        "a step that runs past its time stops the plugin"
    );
}

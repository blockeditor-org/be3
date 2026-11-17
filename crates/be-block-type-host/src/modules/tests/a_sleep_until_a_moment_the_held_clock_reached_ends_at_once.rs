use std::time::Duration;

use super::*;

const SLEEP_UNTIL: &str = "be3:block-type/host@0.1.0#sleep-until";

/// Answers the host's start, then on its next step sleeps until the clock's start, which the
/// held clock has long reached; woken from that, it sleeps until the end of time, which the host
/// holds for it.
struct Sleeper {
    waker: Waker,
    answering: Answering,
    steps: usize,
    sleep: Option<OpId>,
}

impl Sleeper {
    fn sleep_until(&mut self, at: u64) -> OpId {
        self.answering.connection.start(
            None,
            method_id(SLEEP_UNTIL),
            be_sdk_type_api::ops::encode(&at),
        )
    }
}

impl Guest for Sleeper {
    fn start(waker: Waker, _: &mut Vec<Vec<u8>>) -> Result<Self, String> {
        Ok(Self {
            waker,
            answering: Answering::new(FIRST),
            steps: 0,
            sleep: None,
        })
    }

    fn step(
        &mut self,
        inbound: Vec<Vec<u8>>,
        _: bool,
        outbound: &mut Vec<Vec<u8>>,
    ) -> Result<Flow, String> {
        self.steps += 1;
        for frame in &inbound {
            self.answering.connection.receive(frame).unwrap();
        }
        while let Some(event) = self.answering.connection.next_event() {
            match event {
                Incoming::Started(started) if started.method == method_id(START) => {
                    let connection = &mut self.answering.connection;
                    connection.complete(started.op, Outcome::returned(&types(FIRST)));
                }
                Incoming::Completed { op, .. } if Some(op) == self.sleep => {
                    self.sleep = Some(self.sleep_until(u64::MAX));
                }
                _ => {}
            }
        }
        match self.steps {
            1 => self.waker.wake(),
            2 => self.sleep = Some(self.sleep_until(0)),
            _ => {}
        }
        outbound.extend(self.answering.connection.take_outbound());
        Ok(Flow::Running)
    }
}

#[test]
fn a_sleep_until_a_moment_the_held_clock_reached_ends_at_once() {
    be_lib_build_id::install("test");
    let mut modules = Modules::new(
        vec![in_process::<Sleeper>("sleeper").declaring(vec![FIRST])],
        be_block_client::Clock::real(),
    );
    let first = Instant::now();
    modules.hold_clock(Some(first));
    modules.launch_all();
    futures_util::FutureExt::now_or_never(modules.ready()).expect("the module said hello");
    modules.tick_at(0, first);
    // The clock is held a second on, which the module has not been told.
    modules.hold_clock(Some(first + Duration::from_secs(1)));
    let now = modules.now();
    for _ in 0..10 {
        modules.take();
    }
    let held = &modules.modules[0];
    assert_eq!(
        held.sleeping.as_ref().map(|(at, _)| *at),
        Some(u64::MAX),
        "a sleep until a moment the held clock had reached was never answered"
    );
    assert_eq!(
        held.ticked,
        Some(now),
        "the module was told the clock as its sleep ended"
    );
}

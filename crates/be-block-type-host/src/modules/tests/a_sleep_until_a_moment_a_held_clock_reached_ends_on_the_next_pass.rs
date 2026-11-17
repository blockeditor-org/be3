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
fn a_sleep_until_a_moment_a_held_clock_reached_ends_on_the_next_pass() {
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
    // As the host's loop does: it wakes for the sleep's moment (`wake_at`) and ends what is due.
    for _ in 0..10 {
        modules.take();
        modules.tick_due();
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

const CLOCK: &str = "be3:block-type/host@0.1.0#clock";
const ERROR: &str = "be3:block-type/host@0.1.0#error";

/// Follows the host's clock, then sleeps until the moment it was last told; on the step its
/// sleep ends it says, through `error`, which clock values reached it in that same step.
struct Ticked {
    waker: Waker,
    answering: Answering,
    steps: usize,
    clock: Option<OpId>,
    told: Option<u64>,
    sleep: Option<OpId>,
}

impl Guest for Ticked {
    fn start(waker: Waker, _: &mut Vec<Vec<u8>>) -> Result<Self, String> {
        Ok(Self {
            waker,
            answering: Answering::new(FIRST),
            steps: 0,
            clock: None,
            told: None,
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
        let connection = &mut self.answering.connection;
        for frame in &inbound {
            connection.receive(frame).unwrap();
        }
        let (mut ticks, mut woke) = (Vec::new(), false);
        while let Some(event) = connection.next_event() {
            match event {
                Incoming::Started(started) if started.method == method_id(START) => {
                    connection.complete(started.op, Outcome::returned(&types(FIRST)));
                }
                Incoming::Yielded { op, item } if Some(op) == self.clock => {
                    connection.credit(op, 1, item.len() as u64);
                    let now: u64 = be_sdk_type_api::ops::decode(&item).unwrap();
                    self.told = Some(now);
                    ticks.push(now);
                }
                Incoming::Completed { op, .. } if Some(op) == self.sleep => woke = true,
                _ => {}
            }
        }
        if self.steps == 1 {
            self.clock = Some(connection.start(
                None,
                method_id(CLOCK),
                be_sdk_type_api::ops::encode(&()),
            ));
            self.waker.wake();
        } else if self.sleep.is_none()
            && let Some(at) = self.told
        {
            self.sleep = Some(connection.start(
                None,
                method_id(SLEEP_UNTIL),
                be_sdk_type_api::ops::encode(&at),
            ));
        }
        if woke {
            let _ = connection.start(
                None,
                method_id(ERROR),
                be_sdk_type_api::ops::encode(&format!("{ticks:?}")),
            );
        }
        outbound.extend(connection.take_outbound());
        Ok(Flow::Running)
    }
}

/// A module told the held moment that sleeps until it is woken on the host's next pass with
/// the clock yielded again in the same step: it runs its timers as a clock value arrives, so a
/// sleep ended without one would leave them waiting.
#[test]
fn a_due_sleep_ends_with_the_clock_yielded_again() {
    be_lib_build_id::install("test");
    let mut modules = Modules::new(
        vec![in_process::<Ticked>("ticked").declaring(vec![FIRST])],
        be_block_client::Clock::real(),
    );
    let held = Instant::now();
    modules.hold_clock(Some(held));
    modules.launch_all();
    futures_util::FutureExt::now_or_never(modules.ready()).expect("the module said hello");
    let now = modules.now();
    for _ in 0..10 {
        modules.take();
    }
    assert!(
        modules.modules[0].sleeping.is_some(),
        "the module sleeps until the moment it was told"
    );
    assert_eq!(modules.modules[0].ticked, Some(now));

    modules.tick_due();
    let said: Vec<String> = (0..10)
        .flat_map(|_| modules.take())
        .filter_map(|(_, heard)| match heard {
            Heard::Call {
                call: Call::Error { message, .. },
                ..
            } => Some(message),
            _ => None,
        })
        .collect();
    assert_eq!(
        said,
        vec![format!("{:?}", vec![now])],
        "the sleep ended in the same step as the clock reached the module again"
    );
}

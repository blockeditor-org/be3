use std::sync::atomic::{AtomicBool, Ordering};

use super::*;

const SLEEP_UNTIL: &str = "be3:block-type/host@0.1.0#sleep-until";

static WOKE: AtomicBool = AtomicBool::new(false);

/// Answers the host's start, then on its next step sleeps until the clock's start, which the
/// held clock has long reached.
struct Sleeper {
    waker: Waker,
    answering: Answering,
    steps: usize,
    sleep: Option<OpId>,
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
        let connection = &mut self.answering.connection;
        for frame in &inbound {
            connection.receive(frame).unwrap();
        }
        while let Some(event) = connection.next_event() {
            match event {
                Incoming::Started(started) if started.method == method_id(START) => {
                    connection.complete(started.op, Outcome::returned(&types(FIRST)));
                }
                Incoming::Completed { op, .. } if Some(op) == self.sleep => {
                    WOKE.store(true, Ordering::SeqCst);
                }
                _ => {}
            }
        }
        match self.steps {
            1 => self.waker.wake(),
            2 => {
                let at: u64 = 0;
                self.sleep = Some(connection.start(
                    None,
                    method_id(SLEEP_UNTIL),
                    be_sdk_type_api::ops::encode(&at),
                ));
            }
            _ => {}
        }
        outbound.extend(connection.take_outbound());
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
    let held = Instant::now();
    modules.hold_clock(Some(held));
    modules.launch_all();
    futures_util::FutureExt::now_or_never(modules.ready()).expect("the module said hello");
    // The module has seen the held moment, so the clock does not move again.
    modules.tick_at(0, held);
    for _ in 0..10 {
        modules.take();
        modules.tick_due();
    }
    assert!(
        WOKE.load(Ordering::SeqCst),
        "a sleep until a moment the held clock had reached was never answered"
    );
}

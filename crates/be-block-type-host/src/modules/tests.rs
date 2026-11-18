use be_sdk_type_api::{
    Description, Plain, TypeInfo,
    ops::{Connection, Incoming, Outcome, Side, method_id},
};
use be_wasm_guest_abi::{Flow, Guest, Waker};
use uuid::Uuid;

use super::*;
use std::time::Duration;

mod a_declared_module_starts_only_when_its_type_is_first_needed;
mod a_due_sleep_ends_with_the_clock_yielded_again;
mod a_module_that_says_hello_while_another_is_asked_is_still_registered;
mod a_sleep_until_a_moment_a_held_clock_reached_ends_on_the_next_pass;
mod every_module_that_registers_kinds_tells_the_host;

const START: &str = "be3:block-type/replicas@0.1.0#start";

fn types(content_type: Uuid) -> Vec<TypeInfo> {
    vec![TypeInfo {
        content_type,
        frozen: false,
        schema: String::new(),
        undo: false,
        plain: Plain::None,
        first_part: false,
        merges_parts: false,
        whole_bytes: true,
        blank: Vec::new(),
        blank_description: Description::default(),
        reads: None,
    }]
}

struct Answering {
    connection: Connection,
    content_type: Uuid,
}

impl Answering {
    fn new(content_type: Uuid) -> Self {
        Self {
            connection: Connection::new(Side::Guest),
            content_type,
        }
    }

    fn receive(&mut self, inbound: Vec<Vec<u8>>) {
        for frame in inbound {
            self.connection.receive(&frame).unwrap();
        }
        while let Some(event) = self.connection.next_event() {
            let Incoming::Started(started) = event else {
                continue;
            };
            let method = started.method;
            let outcome = if method == method_id(START) {
                Outcome::returned(&types(self.content_type))
            } else {
                Outcome::Err(OpError::Unsupported)
            };
            self.connection.complete(started.op, outcome);
        }
    }
}

const FIRST: Uuid = Uuid::from_u128(1);
const SECOND: Uuid = Uuid::from_u128(2);

struct Slow {
    waker: Waker,
    answering: Answering,
    held: Vec<Vec<u8>>,
}

impl Guest for Slow {
    fn start(waker: Waker, _: &mut Vec<Vec<u8>>) -> Result<Self, String> {
        Ok(Self {
            waker,
            answering: Answering::new(FIRST),
            held: Vec::new(),
        })
    }

    fn step(
        &mut self,
        inbound: Vec<Vec<u8>>,
        _: bool,
        outbound: &mut Vec<Vec<u8>>,
    ) -> Result<Flow, String> {
        outbound.append(&mut self.held);
        self.answering.receive(inbound);
        self.held = self.answering.connection.take_outbound();
        if !self.held.is_empty() {
            self.waker.wake();
        }
        Ok(Flow::Running)
    }
}

struct Late {
    waker: Waker,
    answering: Answering,
    steps: usize,
    held: Vec<Vec<u8>>,
}

impl Guest for Late {
    fn start(waker: Waker, _: &mut Vec<Vec<u8>>) -> Result<Self, String> {
        Ok(Self {
            waker,
            answering: Answering::new(SECOND),
            steps: 0,
            held: Vec::new(),
        })
    }

    fn step(
        &mut self,
        inbound: Vec<Vec<u8>>,
        _: bool,
        outbound: &mut Vec<Vec<u8>>,
    ) -> Result<Flow, String> {
        self.steps += 1;
        self.held.extend(inbound);
        match self.steps {
            1 => self.waker.wake(),
            _ => self.answering.receive(std::mem::take(&mut self.held)),
        }
        outbound.extend(self.answering.connection.take_outbound());
        Ok(Flow::Running)
    }
}

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
            self.clock =
                Some(connection.start(None, method_id(CLOCK), be_sdk_type_api::ops::encode(&())));
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

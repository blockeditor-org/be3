use block_guest_abi::{Flow, Guest, Waker};
use block_type_api::{
    Description, Plain, TypeInfo,
    ops::{Connection, Incoming, Outcome, Side, method_id},
};
use uuid::Uuid;

use super::*;

mod a_module_that_says_hello_while_another_is_asked_is_still_registered;

const START: &str = "be3:block-type/replicas@0.1.0#start";
const BLANK: &str = "be3:block-type/replicas@0.1.0#blank";
const DESCRIBE: &str = "be3:block-type/replicas@0.1.0#describe";

fn types(content_type: Uuid) -> Vec<TypeInfo> {
    vec![TypeInfo {
        content_type,
        frozen: false,
        schema: String::new(),
        undo: false,
        plain: Plain::None,
    }]
}

struct Answering {
    connection: Connection,
    content_type: Uuid,
    started: Vec<u64>,
}

impl Answering {
    fn new(content_type: Uuid) -> Self {
        Self {
            connection: Connection::new(Side::Guest),
            content_type,
            started: Vec::new(),
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
            if started.method == method_id(START) {
                self.started.push(started.op);
                continue;
            }
            let outcome = if started.method == method_id(BLANK) {
                Outcome::returned(&Ok::<Vec<u8>, Failure>(Vec::new()))
            } else if started.method == method_id(DESCRIBE) {
                Outcome::returned(&Ok::<Description, Failure>(Description::default()))
            } else {
                Outcome::Err(OpError::Unsupported)
            };
            self.connection.complete(started.op, outcome);
        }
    }

    fn hello(&mut self) {
        for op in std::mem::take(&mut self.started) {
            self.connection
                .complete(op, Outcome::returned(&types(self.content_type)));
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
        self.answering.hello();
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
}

impl Guest for Late {
    fn start(waker: Waker, _: &mut Vec<Vec<u8>>) -> Result<Self, String> {
        Ok(Self {
            waker,
            answering: Answering::new(SECOND),
            steps: 0,
        })
    }

    fn step(
        &mut self,
        inbound: Vec<Vec<u8>>,
        _: bool,
        outbound: &mut Vec<Vec<u8>>,
    ) -> Result<Flow, String> {
        self.steps += 1;
        self.answering.receive(inbound);
        match self.steps {
            1 => self.waker.wake(),
            2 => self.answering.hello(),
            _ => {}
        }
        outbound.extend(self.answering.connection.take_outbound());
        Ok(Flow::Running)
    }
}

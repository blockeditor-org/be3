use block_guest_abi::{Flow, Guest, Waker};
use block_type_api::{Command, Description, Event, Plain, Reply, Request, TypeInfo};
use block_type_api::{decode_frame, encode_frame};
use uuid::Uuid;

use super::*;

mod a_module_that_says_hello_while_another_is_asked_is_still_registered;

fn hello(content_type: Uuid) -> Vec<u8> {
    encode_frame(&Command::Hello {
        types: vec![TypeInfo {
            content_type,
            frozen: false,
            schema: String::new(),
            undo: false,
            plain: Plain::None,
        }],
    })
}

fn answers(inbound: Vec<Vec<u8>>) -> Vec<Vec<u8>> {
    inbound
        .iter()
        .filter_map(|frame| match decode_frame::<Event>(frame) {
            Ok(Event::Request { id, request }) => Some((id, request)),
            _ => None,
        })
        .map(|(id, request)| {
            let reply = match request {
                Request::Blank { .. } => Reply::Blank(Vec::new()),
                Request::Describe { .. } => Reply::Described(Description::default()),
                _ => Reply::Unknown,
            };
            encode_frame(&Command::Reply { id, reply })
        })
        .collect()
}

const FIRST: Uuid = Uuid::from_u128(1);
const SECOND: Uuid = Uuid::from_u128(2);

struct Slow {
    answering: Vec<Vec<u8>>,
}

impl Guest for Slow {
    fn start(_: Waker, outbound: &mut Vec<Vec<u8>>) -> Result<Self, String> {
        outbound.push(hello(FIRST));
        Ok(Self {
            answering: Vec::new(),
        })
    }

    fn step(
        &mut self,
        inbound: Vec<Vec<u8>>,
        _: bool,
        outbound: &mut Vec<Vec<u8>>,
    ) -> Result<Flow, String> {
        outbound.append(&mut self.answering);
        self.answering = answers(inbound);
        Ok(Flow::Running)
    }
}

struct Late {
    waker: Waker,
    steps: usize,
}

impl Guest for Late {
    fn start(waker: Waker, _: &mut Vec<Vec<u8>>) -> Result<Self, String> {
        Ok(Self { waker, steps: 0 })
    }

    fn step(
        &mut self,
        inbound: Vec<Vec<u8>>,
        _: bool,
        outbound: &mut Vec<Vec<u8>>,
    ) -> Result<Flow, String> {
        self.steps += 1;
        match self.steps {
            1 => self.waker.wake(),
            2 => outbound.push(hello(SECOND)),
            _ => {}
        }
        outbound.extend(answers(inbound));
        Ok(Flow::Running)
    }
}

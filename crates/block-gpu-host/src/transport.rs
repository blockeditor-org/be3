use std::collections::VecDeque;

use block_gpu_abi::embed::device::Caller;

#[derive(Default)]
pub struct Transport {
    pub inbox: VecDeque<Vec<u8>>,
    pub outbox: Vec<Vec<u8>>,
}

pub trait Transported: Caller {
    fn transport(&mut self) -> &mut Transport;
}

pub fn host_send(caller: &mut impl Transported, pointer: u32, length: u32) {
    match caller.read(pointer, length) {
        Ok(frame) => caller.transport().outbox.push(frame),
        Err(message) => caller.refuse(format!("host_send: {message}")),
    }
}

pub fn host_receive(caller: &mut impl Transported, pointer: u32, capacity: u32) -> i64 {
    let Some(needed) = caller.transport().inbox.front().map(Vec::len) else {
        return block_guest_abi::NO_MESSAGE;
    };
    if needed as u32 > capacity {
        return needed as i64;
    }
    let frame = caller.transport().inbox.pop_front().unwrap_or_default();
    if let Err(message) = caller.write(pointer, &frame) {
        caller.refuse(format!("host_receive: {message}"));
    }
    needed as i64
}

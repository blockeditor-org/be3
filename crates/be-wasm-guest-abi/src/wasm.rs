use std::{
    cell::RefCell,
    sync::atomic::{AtomicBool, Ordering},
    thread::LocalKey,
};

use crate::{Flow, Guest, NO_MESSAGE, Waker};

macro_rules! transport_imports {
    ($(fn $name:ident($($argument:ident: $kind:ty),* $(,)?) $(-> $result:ty)?;)+) => {
        #[link(wasm_import_module = "be3_host")]
        unsafe extern "C" {
            $(fn $name($($argument: $kind),*) $(-> $result)?;)+
        }
    };
}

crate::transport_functions!(transport_imports);

// unsafe(ffi)
#[link(wasm_import_module = "be3_host")]
unsafe extern "C" {
    fn host_wake();
    fn host_panic(pointer: u32, length: u32);
}

const RECEIVE_BUFFER: usize = 4096;

static WOKEN: AtomicBool = AtomicBool::new(false);

pub type Slot<G> = LocalKey<RefCell<Option<G>>>;

// unsafe(ffi)
pub fn report_panics() {
    std::panic::set_hook(Box::new(|info| {
        let message = info.payload_as_str().unwrap_or("a panic without a message");
        let report = match info.location() {
            Some(at) => format!(
                "{message}\n(at {}:{}:{})",
                at.file(),
                at.line(),
                at.column()
            ),
            None => message.to_owned(),
        };
        unsafe { host_panic(report.as_ptr() as u32, report.len() as u32) };
    }));
}

// unsafe(ffi)
pub fn send(frame: &[u8]) {
    unsafe { host_send(frame.as_ptr() as u32, frame.len() as u32) };
}

// unsafe(ffi)
pub fn receive() -> Option<Vec<u8>> {
    let mut buffer = vec![0u8; RECEIVE_BUFFER];
    let needed = unsafe { host_receive(buffer.as_mut_ptr() as u32, buffer.len() as u32) };
    if needed == NO_MESSAGE {
        return None;
    }
    let needed = needed as usize;
    if needed > buffer.len() {
        buffer = vec![0u8; needed];
        let again = unsafe { host_receive(buffer.as_mut_ptr() as u32, buffer.len() as u32) };
        if again == NO_MESSAGE {
            return None;
        }
    }
    buffer.truncate(needed);
    Some(buffer)
}

// unsafe(ffi)
pub fn waker() -> Waker {
    Waker::new(|| {
        WOKEN.store(true, Ordering::Release);
        unsafe { host_wake() };
    })
}

pub fn start<G: Guest>(slot: &'static Slot<G>) {
    report_panics();
    let mut outbound = Vec::new();
    let started = G::start(waker(), &mut outbound);
    post(outbound);
    match started {
        Ok(guest) => slot.with(|slot| *slot.borrow_mut() = Some(guest)),
        Err(error) => panic!("{error}"),
    }
}

pub fn step<G: Guest>(slot: &'static Slot<G>) {
    let mut inbound = Vec::new();
    while let Some(frame) = receive() {
        inbound.push(frame);
    }
    let woken = WOKEN.swap(false, Ordering::AcqRel);
    let mut outbound = Vec::new();
    let outcome = slot.with(|slot| match slot.borrow_mut().as_mut() {
        Some(guest) => guest.step(inbound, woken, &mut outbound),
        None => Ok(Flow::Running),
    });
    post(outbound);
    match outcome {
        Ok(Flow::Running) => {}
        Ok(Flow::Finished) => shutdown(slot),
        Err(error) => {
            shutdown(slot);
            panic!("{error}");
        }
    }
}

pub fn shutdown<G: Guest>(slot: &'static Slot<G>) {
    let guest = slot.with(|slot| slot.borrow_mut().take());
    drop(guest);
}

fn post(frames: Vec<Vec<u8>>) {
    for frame in frames {
        send(&frame);
    }
}

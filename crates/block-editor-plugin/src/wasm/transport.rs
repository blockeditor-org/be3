use std::cell::RefCell;
use std::sync::atomic::{AtomicBool, Ordering};

use block_plugin_api::{ErrorCode, Message, ProtocolError, decode_frame, encode_frame};

use crate::{Waker, runtime::Runtime, wasm::host, wasm::surface};

thread_local! {
    static PLUGIN: RefCell<Option<Runtime>> = const { RefCell::new(None) };
}

static WOKEN: AtomicBool = AtomicBool::new(false);

pub(crate) fn start<P: crate::Plugin>(id: &str, name: &str, version: &str) -> Result<(), String> {
    let runtime = Runtime::new::<P>(id, name, version, waker());
    surface::initialize()?;
    post(vec![runtime.hello()]);
    PLUGIN.with(|plugin| *plugin.borrow_mut() = Some(runtime));
    Ok(())
}

pub(crate) fn step() -> Result<(), String> {
    let mut batch = Vec::new();
    while let Some(frame) = host::receive() {
        batch.push(decode_frame(&frame).map_err(|error| format!("{error:?}"))?);
    }
    let woken = WOKEN.swap(false, Ordering::AcqRel);
    let outcome = PLUGIN.with(|plugin| {
        let mut plugin = plugin.borrow_mut();
        let Some(runtime) = plugin.as_mut() else {
            return Ok(false);
        };
        match runtime.step(batch, woken) {
            Ok(step) => {
                post(step.outbound);
                Ok(step.closed)
            }
            Err(error) => {
                post(vec![protocol_error(error.clone())]);
                Err(error)
            }
        }
    });
    match outcome {
        Ok(true) => {
            shutdown();
            Ok(())
        }
        Ok(false) => Ok(()),
        Err(error) => {
            shutdown();
            Err(error)
        }
    }
}

pub(crate) fn initialize_storage(size: usize, align: usize) {
    wasi_threads::initialize_main_thread_storage(size, align);
}

pub(crate) fn shutdown() {
    PLUGIN.with(|plugin| plugin.borrow_mut().take());
}

fn waker() -> Waker {
    Waker::new(|| {
        WOKEN.store(true, Ordering::Release);
        host::wake();
    })
}

fn post(messages: Vec<Message>) {
    for message in messages {
        match encode_frame(&message) {
            Ok(frame) => host::send(&frame),
            Err(error) => eprintln!("dropped a message the protocol refused to carry: {error}"),
        }
    }
}

fn protocol_error(message: String) -> Message {
    Message::Error(ProtocolError {
        request_id: None,
        code: ErrorCode::Internal,
        message,
    })
}

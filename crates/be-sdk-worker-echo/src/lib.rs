//! The smallest worker (be_sdk_worker::Work, exported by worker.rs), which be-wasm-sandbox-host's
//! tests spawn as the echo app's: it says it started, then answers each message with `ECHOED`
//! followed by it, except `STOP`, which stops it.

use be_sdk_worker::{App, Work};

/// The module's name, as an app's manifest names it under `workers`.
pub const MODULE: &str = "echo-worker.wasm";

/// What the worker sends as it starts.
pub const STARTED: &[u8] = b"the worker started";

/// What the worker puts before each message it answers.
pub const ECHOED: &[u8] = b"echoed ";

/// The message that stops the worker, with `STOPPED` as its error.
pub const STOP: &[u8] = b"stop";

/// Why the worker stopped when it was sent `STOP`.
pub const STOPPED: &str = "the echo worker was asked to stop";

pub struct Echo;

impl Work for Echo {
    fn start(app: &mut App) -> Result<Self, String> {
        app.send(STARTED.to_vec());
        Ok(Self)
    }

    fn receive(&mut self, message: Vec<u8>, app: &mut App) -> Result<(), String> {
        if message == STOP {
            return Err(STOPPED.to_owned());
        }
        app.send([ECHOED, &message].concat());
        Ok(())
    }
}

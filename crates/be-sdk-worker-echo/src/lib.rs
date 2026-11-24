//! The smallest worker (be_sdk_worker::Work, exported by worker.rs), which be-wasm-sandbox-host's
//! tests spawn as the echo app's: it says it started, then answers each message with `ECHOED`
//! followed by it, except `STOP`, which stops it, `TWICE`, which it answers twice, and `SILENT`,
//! which it does not answer.

use be_sdk_worker::{App, Work};

/// The module's name, as an app's manifest names it under `workers`.
pub const MODULE: &str = "echo-worker.wasm";

/// What the worker sends as it starts.
pub const STARTED: &[u8] = b"the worker started";

/// What the worker puts before each message it answers.
pub const ECHOED: &[u8] = b"echoed ";

/// The message that stops the worker, with `STOPPED` as its error.
pub const STOP: &[u8] = b"stop";

/// A message the worker answers twice.
pub const TWICE: &[u8] = b"twice";

/// A message the worker does not answer.
pub const SILENT: &[u8] = b"silent";

/// Why the worker stopped when it was sent `STOP`.
pub const STOPPED: &str = "the echo worker was asked to stop";

pub struct Echo;

impl Work for Echo {
    type In = Vec<u8>;
    type Out = Vec<u8>;

    fn start(app: &mut App<Vec<u8>>) -> Result<Self, String> {
        app.send(&STARTED.to_vec());
        Ok(Self)
    }

    fn receive(&mut self, message: Vec<u8>, app: &mut App<Vec<u8>>) -> Result<(), String> {
        if message == STOP {
            return Err(STOPPED.to_owned());
        }
        if message == SILENT {
            return Ok(());
        }
        app.send(&[ECHOED, &message].concat());
        if message == TWICE {
            app.send(&[ECHOED, &message].concat());
        }
        Ok(())
    }
}

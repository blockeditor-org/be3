//! The smallest worker (be_sdk_worker::worker!), which be-wasm-sandbox-host's tests spawn as the
//! echo app's: it says it started, then answers each message with `ECHOED` followed by it.

use be_sdk_worker::{App, Work};

/// The module's name, as an app's manifest names it under `workers`.
pub const MODULE: &str = "echo-worker.wasm";

/// What the worker sends as it starts.
pub const STARTED: &[u8] = b"the worker started";

/// What the worker puts before each message it answers.
pub const ECHOED: &[u8] = b"echoed ";

pub struct Echo;

impl Work for Echo {
    fn start(app: &mut App) -> Result<Self, String> {
        app.send(STARTED.to_vec());
        Ok(Self)
    }

    fn receive(&mut self, message: Vec<u8>, app: &mut App) -> Result<(), String> {
        app.send([ECHOED, &message].concat());
        Ok(())
    }
}

be_sdk_worker::worker!(Echo);

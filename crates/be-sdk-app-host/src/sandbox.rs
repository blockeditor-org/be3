//! What serves an app the sandbox API (`be3:sandbox`) on its wire: the host's
//! `be_wasm_sandbox_host::Server`, which runs an app's workers each on an instance of its own, or
//! an editor test's in-process workers (be-sdk-test). The wire hands it the app's sandbox calls,
//! the ops the app cancels and a poll whenever the host steps the wire, and stops it with the app.

use be_sdk_api::ops::{Connection, OpId};
use be_wasm_sandbox_api::embed::sandbox::Call;

pub trait Sandbox {
    /// Takes one of the app's sandbox calls, answering it on `connection` now or later.
    fn call(&mut self, call: Call, connection: &mut Connection);

    /// An op the app canceled; answers whether it was the sandbox's.
    fn canceled(&mut self, op: OpId, connection: &mut Connection) -> bool;

    /// Answers whatever its instances have for the app since.
    fn poll(&mut self, connection: &mut Connection);

    /// Stops every instance, answering each `next` the app waits on with `why`.
    fn stop(self: Box<Self>, why: &str, connection: &mut Connection);
}

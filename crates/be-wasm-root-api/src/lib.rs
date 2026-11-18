//! The root guest's entry (plans/host_as_platform_adapter.md, The root guest): the WIT world
//! `be3:root/root` and the Rust generated from it. The root exports `frames`, whose `frame` the
//! host calls inline on its frame thread and steps until it answers (crates/be-wasm-root-host
//! natively), `input`, whose `deliver` hands it the window's input to route to its surfaces,
//! and `paintings`, what its own surfaces painted, for the host's tests; it imports `host`'s
//! `deadline` and `fonts`; the host also serves it the sandbox (crates/be-wasm-sandbox-api).

pub use be_wasm_ops as ops;

be_wasm_ops_bindgen::generate!({
    path: "wit",
    world: "root",
});

pub use frames::{FrameDone, FrameRequest};
pub use host::{
    LockState, PowerAction,HostStatistics, Notice, NoticeAction, ShownNotices, StatisticRow};

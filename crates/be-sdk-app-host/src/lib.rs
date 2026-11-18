//! The host's half of the app protocol (`be3:app`, crates/be-sdk-api), as a library any host
//! links, natively or as wasm: be-host-runtime, be-sdk-test's in-process host, and the root
//! guest (plans/host_as_platform_adapter.md, step 4e). It knows no GUI framework, as the protocol
//! does not (AGENTS.md, Apps), and no block stack.

pub mod accessibility;
pub mod blocks;
pub mod calls;
pub mod docks;
pub mod holds;
pub mod pacing;
pub mod published;
pub mod sandbox;
pub mod sent;
pub mod session;
pub mod wire;

pub mod bell;
mod in_process;
#[cfg(not(target_arch = "wasm32"))]
pub mod thread;
#[cfg(target_arch = "wasm32")]
pub mod web;
mod worker;

#[cfg(test)]
mod tests;

use std::sync::Arc;

pub use in_process::InProcess;
pub use worker::{
    Channel, Deadline, Event, NOT_RESPONDING, Presented, Produced, Received, STEP_TIMEOUT, STOPPED,
    StepTime, Worker, wakes,
};

pub type Wake = Arc<dyn Fn() + Send + Sync>;

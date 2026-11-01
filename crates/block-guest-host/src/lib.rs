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

pub fn panic_message(payload: &(dyn std::any::Any + Send)) -> String {
    if let Some(message) = payload.downcast_ref::<&str>() {
        return (*message).to_owned();
    }
    if let Some(message) = payload.downcast_ref::<String>() {
        return message.clone();
    }
    "a panic without a message".to_owned()
}

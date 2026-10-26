mod simulation;
pub mod testing;

pub use simulation::Simulation;

#[cfg(not(target_arch = "wasm32"))]
mod platform;
#[cfg(not(target_arch = "wasm32"))]
pub use platform::Headless;

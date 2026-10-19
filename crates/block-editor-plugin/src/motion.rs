pub use block_plugin_api::Motion;
use std::cell::Cell;
thread_local! { static MOTION: Cell<Motion> = const { Cell::new(Motion::Animated) }; }
pub(crate) fn receive(motion: Motion) {
    MOTION.with(|held| held.set(motion));
}
pub fn motion() -> Motion {
    MOTION.with(Cell::get)
}

#[cfg(test)]
mod tests;

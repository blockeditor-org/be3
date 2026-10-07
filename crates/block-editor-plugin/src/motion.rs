use std::cell::Cell;

use block_plugin_api::Motion;

thread_local! {
    static MOTION: Cell<Motion> = const { Cell::new(Motion::Animated) };
}

pub(crate) fn receive(motion: Motion) {
    MOTION.with(|held| held.set(motion));
}

pub fn motion() -> Motion {
    MOTION.with(Cell::get)
}

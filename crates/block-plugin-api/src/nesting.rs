use crate::MAX_MENU_DEPTH;
use serde::de::Error;
use serde::{Deserialize, Deserializer};
use std::cell::Cell;

pub(crate) const TOO_DEEP: &str = "menu nested too deeply";

thread_local! {
    static DEPTH: Cell<usize> = const { Cell::new(0) };
}

struct Level;

impl Level {
    fn enter() -> (Self, usize) {
        let depth = DEPTH.with(|depth| {
            depth.set(depth.get() + 1);
            depth.get()
        });
        (Self, depth)
    }
}

impl Drop for Level {
    fn drop(&mut self) {
        DEPTH.with(|depth| depth.set(depth.get().saturating_sub(1)));
    }
}

pub(crate) fn submenu<'de, D, T>(deserializer: D) -> Result<Vec<T>, D::Error>
where
    D: Deserializer<'de>,
    T: Deserialize<'de>,
{
    let (_level, depth) = Level::enter();
    if depth > MAX_MENU_DEPTH {
        return Err(D::Error::custom(TOO_DEEP));
    }
    Vec::deserialize(deserializer)
}

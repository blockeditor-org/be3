use std::cell::Cell;

use block_plugin_api::Preferences;

thread_local! {
    static PREFERENCES: Cell<Preferences> = Cell::new(Preferences::default());
}

pub(crate) fn receive(preferences: Preferences) {
    PREFERENCES.with(|held| held.set(preferences));
}

pub fn host_preferences() -> Preferences {
    PREFERENCES.with(Cell::get)
}

#[cfg(test)]
mod tests;

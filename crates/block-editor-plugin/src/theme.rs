use std::cell::Cell;

use block_plugin_api::Theme;

thread_local! {
    static THEME: Cell<Theme> = const { Cell::new(Theme::Dark) };
}

pub(crate) fn receive(theme: Theme) {
    THEME.with(|held| held.set(theme));
}

pub fn host_theme() -> Theme {
    THEME.with(Cell::get)
}

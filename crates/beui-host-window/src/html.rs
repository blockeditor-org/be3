use std::error::Error;

use beui::styled::Root;

/// What a program asks of where it runs. On beui's HTML core the page is the program's
/// window, so nothing here changes how it runs yet.
#[derive(Default)]
pub struct RunOptions {}

impl RunOptions {
    /// Where the native core keeps the program's preferences between runs. A browser keeps
    /// none for it yet, as on the native core's web.
    pub fn saving_preferences(self, _program: &str) -> Self {
        self
    }
}

pub(crate) fn run(_options: RunOptions, root: impl FnOnce() -> Root) -> Result<(), Box<dyn Error>> {
    Ok(beui::html::run(&crate::web::element(), root)?)
}

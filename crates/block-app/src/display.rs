use be_block::DisplaySettings;

use crate::app_state::{AppStateError, AppStateStore};
use crate::local_settings::LocalSettings;

#[cfg(target_os = "linux")]
mod linux;
#[cfg(target_os = "linux")]
use linux::apply;
#[cfg(target_os = "linux")]
pub(crate) use linux::start;

#[cfg(not(target_os = "linux"))]
pub(crate) fn start(_setup: &beui::Setup) {}

#[cfg(not(target_os = "linux"))]
fn apply(_settings: &DisplaySettings) {}

impl LocalSettings for DisplaySettings {
    const NAME: &'static str = "display settings";

    fn load(store: &AppStateStore) -> Result<Option<Vec<u8>>, AppStateError> {
        store.display_settings()
    }

    fn save(store: &AppStateStore, content: &[u8]) -> Result<(), AppStateError> {
        store.set_display_settings(content)
    }

    fn apply(&self) {
        apply(self);
    }
}

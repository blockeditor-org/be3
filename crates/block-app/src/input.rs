use be_block::InputSettings;

use crate::local_settings::LocalSettings;
use block_app_base::app_state::{AppStateError, AppStateStore};

#[cfg(target_os = "linux")]
mod linux;
#[cfg(target_os = "linux")]
use linux::apply;
#[cfg(target_os = "linux")]
pub(crate) use linux::start;

#[cfg(not(target_os = "linux"))]
pub(crate) fn start(_setup: &beui::native::Setup) {}

#[cfg(not(target_os = "linux"))]
fn apply(_settings: &InputSettings) {}

impl LocalSettings for InputSettings {
    fn load(store: &AppStateStore) -> Result<Option<Vec<u8>>, AppStateError> {
        store.input_settings()
    }

    fn save(store: &AppStateStore, content: &[u8]) -> Result<(), AppStateError> {
        store.set_input_settings(content)
    }

    fn apply(&self) -> bool {
        apply(self);
        true
    }
}

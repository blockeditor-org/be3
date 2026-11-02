use be_block::InputSettings;

use crate::local_settings::LocalSettings;
use be_host_base::host_state::{HostStateError, HostStateStore};

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
    fn load(store: &HostStateStore) -> Result<Option<Vec<u8>>, HostStateError> {
        store.input_settings()
    }

    fn save(store: &HostStateStore, content: &[u8]) -> Result<(), HostStateError> {
        store.set_input_settings(content)
    }

    fn apply(&self) -> bool {
        apply(self);
        true
    }
}

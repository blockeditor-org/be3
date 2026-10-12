use be_block::InputSettings;

use crate::app_state::{AppStateError, AppStateStore};
use crate::local_settings::LocalSettings;

use crate::extension::apply_input as apply;

impl LocalSettings for InputSettings {
    const NAME: &'static str = "input settings";

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

use be_block::{BlockContent, SettingsContent};

use crate::BlockApp;

impl BlockApp {
    pub(crate) fn open_settings(&mut self) {
        let Some(id) = self.root_settings.ensure() else {
            return;
        };
        self.show_in_shell(id, SettingsContent::CONTENT_TYPE, None);
    }
}

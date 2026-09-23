use crate::BlockApp;
use block::Block;
use block_client::blocks::settings::Settings;

impl BlockApp {
    pub(crate) fn open_settings(&mut self) {
        let Some(id) = self
            .root_settings
            .ensure(&self.client)
            .map(|settings| settings.id())
        else {
            return;
        };
        self.show_in_shell(id, Settings::TYPE_ID, None);
    }
}

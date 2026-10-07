use be_block::{BlockContent, InputSettings, InputSettingsContent};
use uuid::Uuid;

use crate::app_state::AppStateStore;
use crate::be;

#[cfg(target_os = "linux")]
mod linux;
#[cfg(target_os = "linux")]
pub(crate) use linux::{apply, start};

#[cfg(not(target_os = "linux"))]
pub(crate) fn start(_setup: &beui::Setup) {}

#[cfg(not(target_os = "linux"))]
fn apply(_settings: &InputSettings) {}

#[derive(Default)]
pub(crate) struct InputSync {
    block: Option<Uuid>,
    seen: Option<(Uuid, u64)>,
    applied: Option<InputSettings>,
}

impl InputSync {
    pub(crate) fn boot(&mut self, store: &AppStateStore) {
        match store.input_settings() {
            Ok(Some(content)) => match InputSettingsContent::decode(&content) {
                Ok(content) => {
                    let settings = content.root();
                    apply(&settings);
                    self.applied = Some(settings);
                }
                Err(error) => {
                    eprintln!("block-app: the saved input settings are unreadable: {error}")
                }
            },
            Ok(None) => {}
            Err(error) => eprintln!("block-app: the saved input settings were not read: {error}"),
        }
    }

    pub(crate) fn block(&self) -> Option<Uuid> {
        self.block
    }

    pub(crate) fn set_block(&mut self, block: Option<Uuid>) {
        self.block = block;
        self.seen = None;
    }

    pub(crate) fn sync(&mut self, store: &AppStateStore) {
        let Some(block) = self.block else {
            return;
        };
        be::hold(block, InputSettingsContent::CONTENT_TYPE);
        let Some(revision) = be::content_revision(block) else {
            return;
        };
        if self.seen == Some((block, revision)) {
            return;
        }
        let Some(content) = be::content(block)
            .and_then(|content| InputSettingsContent::decode(&content.bytes).ok())
        else {
            return;
        };
        self.seen = Some((block, revision));
        let settings = content.root();
        if self.applied.as_ref() == Some(&settings) {
            return;
        }
        apply(&settings);
        if let Err(error) = store.set_input_settings(&InputSettingsContent::new(&settings).encode())
        {
            eprintln!("block-app: the input settings were not saved on this device: {error}");
        }
        self.applied = Some(settings);
    }
}

use be_block::be_model::Document;
use be_block::settings::Settings;
use be_block::{BlockContent, Root};
use uuid::Uuid;

use crate::app_state::{AppStateError, AppStateStore};
use crate::be;

pub(crate) trait LocalSettings: Root + Clone + PartialEq {
    const NAME: &'static str;

    fn load(store: &AppStateStore) -> Result<Option<Vec<u8>>, AppStateError>;

    fn save(store: &AppStateStore, content: &[u8]) -> Result<(), AppStateError>;

    fn apply(&self);
}

pub(crate) struct SettingsSync<R> {
    block: Option<Uuid>,
    seen: Option<(Uuid, u64)>,
    applied: Option<R>,
}

impl<R> Default for SettingsSync<R> {
    fn default() -> Self {
        Self {
            block: None,
            seen: None,
            applied: None,
        }
    }
}

impl<R: LocalSettings> SettingsSync<R> {
    pub(crate) fn boot(&mut self, store: &AppStateStore) {
        let name = R::NAME;
        match R::load(store) {
            Ok(Some(content)) => match Document::<R>::decode(&content) {
                Ok(content) => {
                    let settings = content.root();
                    settings.apply();
                    self.applied = Some(settings);
                }
                Err(error) => eprintln!("block-app: the saved {name} are unreadable: {error}"),
            },
            Ok(None) => {}
            Err(error) => eprintln!("block-app: the saved {name} were not read: {error}"),
        }
    }

    pub(crate) fn block(&self) -> Option<Uuid> {
        self.block
    }

    pub(crate) fn resolve(&mut self, settings: &Settings, client_id: Uuid) {
        if self.block.is_none() {
            let block = settings.resolve(R::CONTENT_TYPE, client_id);
            if block.is_some() {
                self.set_block(block);
            }
        }
    }

    pub(crate) fn set_block(&mut self, block: Option<Uuid>) {
        self.block = block;
        self.seen = None;
    }

    pub(crate) fn sync(&mut self, store: &AppStateStore) {
        let Some(block) = self.block else {
            return;
        };
        be::hold(block, R::CONTENT_TYPE);
        let Some(revision) = be::content_revision(block) else {
            return;
        };
        if self.seen == Some((block, revision)) {
            return;
        }
        let Some(content) =
            be::content(block).and_then(|content| Document::<R>::decode(&content.bytes).ok())
        else {
            return;
        };
        self.seen = Some((block, revision));
        let settings = content.root();
        if self.applied.as_ref() == Some(&settings) {
            return;
        }
        settings.apply();
        if let Err(error) = R::save(store, &Document::new(&settings).encode()) {
            eprintln!(
                "block-app: the {} were not saved on this device: {error}",
                R::NAME
            );
        }
        self.applied = Some(settings);
    }
}

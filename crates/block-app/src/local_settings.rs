use be_block::be_model::{Document, Edit};
use be_block::settings::Settings;
use be_block::{BlockContent, LiveEdit, Root};
use uuid::Uuid;

use crate::app_state::{AppStateError, AppStateStore};
use crate::be;

pub(crate) trait LocalSettings: Root + Clone + Default + PartialEq {
    const NAME: &'static str;

    fn load(store: &AppStateStore) -> Result<Option<Vec<u8>>, AppStateError>;

    fn save(store: &AppStateStore, content: &[u8]) -> Result<(), AppStateError>;

    fn apply(&self) -> bool;
}

pub(crate) struct SettingsSync<R> {
    block: Option<Uuid>,
    seen: Option<(Uuid, u64)>,
    applied: Option<R>,
    unwritten: Vec<Edit>,
}

impl<R> Default for SettingsSync<R> {
    fn default() -> Self {
        Self {
            block: None,
            seen: None,
            applied: None,
            unwritten: Vec::new(),
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
        self.write(block);
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
        self.receive(store, content.root());
    }

    fn receive(&mut self, store: &AppStateStore, settings: R) {
        if self.applied.as_ref() == Some(&settings) {
            return;
        }
        if settings.apply() {
            self.save(store, &settings);
        }
        self.applied = Some(settings);
    }

    pub(crate) fn commit(&mut self, store: &AppStateStore, edits: Vec<Edit>) {
        let mut document = Document::new(&self.applied.clone().unwrap_or_default());
        for edit in &edits {
            document.apply(edit);
        }
        let settings = document.root();
        self.save(store, &settings);
        self.applied = Some(settings);
        self.unwritten.extend(edits);
        if let Some(block) = self.block {
            self.write(block);
        }
    }

    fn write(&mut self, block: Uuid) {
        if self.unwritten.is_empty() {
            return;
        }
        let Some(revision) = be::content_revision(block) else {
            return;
        };
        for edit in self.unwritten.drain(..) {
            be::operate(block, Document::<R>::encode_operation(&edit));
        }
        self.seen = Some((block, revision));
    }

    fn save(&self, store: &AppStateStore, settings: &R) {
        if let Err(error) = R::save(store, &Document::new(settings).encode()) {
            eprintln!(
                "block-app: the {} were not saved on this device: {error}",
                R::NAME
            );
        }
    }
}

#[cfg(test)]
mod tests;

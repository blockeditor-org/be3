use be_block::be_model::{Document, Edit};
use be_block::{BlockContent, LiveEdit, Root};
use uuid::Uuid;

use block_app_lib::extension::{self, App};

pub(crate) trait LocalSettings: Root + Clone + Default + PartialEq {
    const NAME: &'static str;

    fn apply(&self) -> bool;
}

pub(crate) trait Cache {
    fn cached_copy(&self, key: &str) -> Result<Option<Vec<u8>>, String>;

    fn cache_copy(&self, key: &str, content: &[u8]) -> Result<(), String>;
}

impl Cache for App<'_> {
    fn cached_copy(&self, key: &str) -> Result<Option<Vec<u8>>, String> {
        App::cached_copy(self, key)
    }

    fn cache_copy(&self, key: &str, content: &[u8]) -> Result<(), String> {
        App::cache_copy(self, key, content)
    }
}

pub(crate) struct SettingsSync<R> {
    block: Option<Uuid>,
    seen: Option<(Uuid, u64)>,
    applied: Option<R>,
    outbox: Outbox,
}

#[derive(Default)]
struct Outbox {
    unwritten: Vec<Edit>,
    sent: Vec<Edit>,
    sent_at: Option<u64>,
}

impl Outbox {
    fn queue(&mut self, edits: Vec<Edit>) {
        self.unwritten.extend(edits);
    }

    fn send(&mut self, revision: u64) -> Vec<Edit> {
        if self.unwritten.is_empty() {
            return Vec::new();
        }
        let edits = std::mem::take(&mut self.unwritten);
        self.sent.extend(edits.iter().cloned());
        self.sent_at = Some(revision);
        edits
    }

    fn overlay<R: LocalSettings>(&mut self, revision: u64, content: R) -> (R, Vec<Edit>) {
        if self.sent.is_empty() {
            return (content, Vec::new());
        }
        let overlaid = edited(&content, &self.sent);
        if overlaid == content {
            self.sent.clear();
            self.sent_at = None;
            return (content, Vec::new());
        }
        let resend = match self.sent_at == Some(revision) {
            true => Vec::new(),
            false => {
                self.sent_at = Some(revision);
                self.sent.clone()
            }
        };
        (overlaid, resend)
    }

    fn clear(&mut self) {
        *self = Self::default();
    }
}

fn edited<R: Root>(settings: &R, edits: &[Edit]) -> R {
    let mut document = Document::new(settings);
    for edit in edits {
        document.apply(edit);
    }
    document.root()
}

impl<R> Default for SettingsSync<R> {
    fn default() -> Self {
        Self {
            block: None,
            seen: None,
            applied: None,
            outbox: Outbox::default(),
        }
    }
}

impl<R: LocalSettings> SettingsSync<R> {
    pub(crate) fn boot(&mut self, cache: &impl Cache) {
        let name = R::NAME;
        match cache.cached_copy(name) {
            Ok(Some(content)) => match Document::<R>::decode(&content) {
                Ok(content) => {
                    let settings = content.root();
                    settings.apply();
                    self.applied = Some(settings);
                }
                Err(error) => eprintln!("block-desktop: the saved {name} are unreadable: {error}"),
            },
            Ok(None) => {}
            Err(error) => eprintln!("block-desktop: the saved {name} were not read: {error}"),
        }
    }

    fn set_block(&mut self, block: Option<Uuid>) {
        if self.block.is_some() && self.block != block {
            self.outbox.clear();
        }
        self.block = block;
        self.seen = None;
    }

    pub(crate) fn sync(&mut self, app: &mut App) {
        let block = app.settings_block(R::CONTENT_TYPE);
        if block != self.block {
            self.set_block(block);
        }
        let Some(block) = block else {
            return;
        };
        extension::hold(block, R::CONTENT_TYPE);
        let Some(revision) = extension::content_revision(block) else {
            return;
        };
        Self::write(block, self.outbox.send(revision));
        if self.seen == Some((block, revision)) {
            return;
        }
        let Some(content) =
            extension::content(block).and_then(|content| Document::<R>::decode(&content).ok())
        else {
            return;
        };
        self.seen = Some((block, revision));
        let (settings, resend) = self.outbox.overlay(revision, content.root());
        Self::write(block, resend);
        self.receive(&*app, settings);
    }

    fn receive(&mut self, cache: &impl Cache, settings: R) {
        if self.applied.as_ref() == Some(&settings) {
            return;
        }
        if settings.apply() {
            self.save(cache, &settings);
        }
        self.applied = Some(settings);
    }

    pub(crate) fn commit(&mut self, cache: &impl Cache, edits: Vec<Edit>) {
        let settings = edited(&self.applied.clone().unwrap_or_default(), &edits);
        self.save(cache, &settings);
        self.applied = Some(settings);
        self.outbox.queue(edits);
        if let Some(block) = self.block
            && let Some(revision) = extension::content_revision(block)
        {
            Self::write(block, self.outbox.send(revision));
        }
    }

    fn write(block: Uuid, edits: Vec<Edit>) {
        for edit in edits {
            extension::operate(block, Document::<R>::encode_operation(&edit));
        }
    }

    fn save(&self, cache: &impl Cache, settings: &R) {
        if let Err(error) = cache.cache_copy(R::NAME, &Document::new(settings).encode()) {
            eprintln!(
                "block-desktop: the {} were not saved on this device: {error}",
                R::NAME
            );
        }
    }
}

#[cfg(test)]
mod tests;

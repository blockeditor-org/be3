use be_block::be_model::{Document, Edit};
use be_block::{BlockContent, Layered, Layers, LiveEdit, Root, SettingsLayer, layered};
use uuid::Uuid;

use crate::app_state::{AppStateError, AppStateStore};
use crate::be;

pub(crate) trait LocalSettings: Root + Clone + Default + PartialEq {
    fn load(store: &AppStateStore) -> Result<Option<Vec<u8>>, AppStateError>;

    fn save(store: &AppStateStore, content: &[u8]) -> Result<(), AppStateError>;

    fn apply(&self) -> bool;
}

pub(crate) struct SettingsSync<R> {
    blocks: Layers,
    seen: Option<Vec<(Uuid, u64)>>,
    contents: Option<LayerContents<R>>,
    applied: Option<R>,
    outboxes: [Outbox; 2],
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct LayerContents<R> {
    pub(crate) every_device: Option<R>,
    pub(crate) this_device: Option<R>,
}

impl<R> Default for LayerContents<R> {
    fn default() -> Self {
        Self {
            every_device: None,
            this_device: None,
        }
    }
}

impl<R: Layered> LayerContents<R> {
    fn get_mut(&mut self, layer: SettingsLayer) -> &mut Option<R> {
        match layer {
            SettingsLayer::EveryDevice => &mut self.every_device,
            SettingsLayer::ThisDevice => &mut self.this_device,
        }
    }

    pub(crate) fn own(&self, layer: SettingsLayer) -> R {
        match layer {
            SettingsLayer::EveryDevice => self.every_device.clone(),
            SettingsLayer::ThisDevice => self.this_device.clone(),
        }
        .unwrap_or_default()
    }

    pub(crate) fn set(&mut self, layer: SettingsLayer, content: R) {
        *self.get_mut(layer) = Some(content);
    }

    fn target(&self, layer: SettingsLayer) -> Option<SettingsLayer> {
        let other = match layer {
            SettingsLayer::EveryDevice => SettingsLayer::ThisDevice,
            SettingsLayer::ThisDevice => SettingsLayer::EveryDevice,
        };
        [layer, other].into_iter().find(|layer| match layer {
            SettingsLayer::EveryDevice => self.every_device.is_some(),
            SettingsLayer::ThisDevice => self.this_device.is_some(),
        })
    }

    fn layered(&self) -> R {
        layered(
            self.every_device
                .iter()
                .chain(self.this_device.iter())
                .cloned(),
        )
    }
}

fn slot(layer: SettingsLayer) -> usize {
    match layer {
        SettingsLayer::EveryDevice => 0,
        SettingsLayer::ThisDevice => 1,
    }
}

const LAYERS: [SettingsLayer; 2] = [SettingsLayer::EveryDevice, SettingsLayer::ThisDevice];

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
            blocks: Layers::default(),
            seen: None,
            contents: None,
            applied: None,
            outboxes: [Outbox::default(), Outbox::default()],
        }
    }
}

impl<R: LocalSettings + Layered> SettingsSync<R> {
    pub(crate) fn boot(&mut self, store: &AppStateStore) {
        let name = <R as Layered>::NAME;
        match R::load(store) {
            Ok(Some(content)) => match Document::<R>::decode(&content) {
                Ok(content) => {
                    let settings = content.root();
                    settings.apply();
                    self.applied = Some(settings);
                }
                Err(error) => {
                    eprintln!("block-app: the saved {name} settings are unreadable: {error}")
                }
            },
            Ok(None) => {}
            Err(error) => eprintln!("block-app: the saved {name} settings were not read: {error}"),
        }
    }

    pub(crate) fn applied(&self) -> Option<&R> {
        self.applied.as_ref()
    }

    pub(crate) fn set_layers(&mut self, layers: Layers) {
        if self.blocks == layers {
            return;
        }
        for layer in LAYERS {
            if self.blocks.block(layer) != layers.block(layer) {
                self.outboxes[slot(layer)].clear();
            }
        }
        self.blocks = layers;
        self.seen = None;
        self.contents = None;
    }

    pub(crate) fn layers(&self) -> LayerContents<R> {
        self.contents.clone().unwrap_or_default()
    }

    pub(crate) fn sync(&mut self, store: &AppStateStore) {
        let blocks = self.blocks;
        if blocks.top().is_none() {
            return;
        }
        let mut revisions = Vec::with_capacity(2);
        for layer in LAYERS {
            let Some(block) = blocks.block(layer) else {
                continue;
            };
            be::hold(block, R::CONTENT_TYPE);
            let Some(revision) = be::content_revision(block) else {
                return;
            };
            revisions.push((block, revision));
            Self::write(block, self.outboxes[slot(layer)].send(revision));
        }
        if self.seen.as_ref() == Some(&revisions) {
            return;
        }
        let mut contents = LayerContents::default();
        for layer in LAYERS {
            let Some(block) = blocks.block(layer) else {
                continue;
            };
            let Some(content) = be::content(block)
                .and_then(|content| Document::<R>::decode(content.bytes()).ok())
                .map(|content| content.root())
            else {
                return;
            };
            let revision = be::content_revision(block).unwrap_or(0);
            let (content, resend) = self.outboxes[slot(layer)].overlay(revision, content);
            Self::write(block, resend);
            contents.set(layer, content);
        }
        self.seen = Some(revisions);
        let settings = contents.layered();
        self.contents = Some(contents);
        self.receive(store, settings);
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

    pub(crate) fn commit(&mut self, store: &AppStateStore, edits: Vec<(SettingsLayer, Edit)>) {
        let mut contents = self.contents.clone().unwrap_or_default();
        let mut applied = self.applied.clone().unwrap_or_default();
        let mut written = false;
        for (layer, edit) in edits {
            match contents.target(layer) {
                Some(target) => {
                    let content = edited(&contents.own(target), std::slice::from_ref(&edit));
                    contents.set(target, content);
                    self.outboxes[slot(target)].queue(vec![edit]);
                    written = true;
                }
                None => {
                    applied = edited(&applied, std::slice::from_ref(&edit));
                    self.outboxes[slot(layer)].queue(vec![edit]);
                }
            }
        }
        let settings = match written {
            true => contents.layered(),
            false => applied,
        };
        if self.contents.is_some() {
            self.contents = Some(contents);
        }
        self.save(store, &settings);
        self.applied = Some(settings);
        for layer in LAYERS {
            if let Some(block) = self.blocks.block(layer)
                && let Some(revision) = be::content_revision(block)
            {
                Self::write(block, self.outboxes[slot(layer)].send(revision));
            }
        }
    }

    fn write(block: Uuid, edits: Vec<Edit>) {
        for edit in edits {
            be::operate(block, Document::<R>::encode_operation(&edit));
        }
    }

    fn save(&self, store: &AppStateStore, settings: &R) {
        if let Err(error) = R::save(store, &Document::new(settings).encode()) {
            eprintln!(
                "block-app: the {} settings were not saved on this device: {error}",
                <R as Layered>::NAME
            );
        }
    }
}

#[cfg(test)]
mod tests;

use std::rc::Rc;

use be_block_content::be_block_model::{Document, Edit};
use be_block_content::{BlockContent, Layered, Layers, LiveEdit, Root, SettingsLayer, layered};
use uuid::Uuid;

use be_host_base::host_state::{HostStateError, HostStateStore};

pub(crate) trait LocalSettings: Root + Clone + Default + PartialEq {
    fn load(store: &HostStateStore) -> Result<Option<Vec<u8>>, HostStateError>;

    fn save(store: &HostStateStore, content: &[u8]) -> Result<(), HostStateError>;

    fn apply(&self) -> bool;
}

pub(crate) struct SettingsSync<R> {
    blocks: Layers,
    /// Whether a layer's block changed since its content was read (`heard`).
    due: bool,
    /// The revision of each layer block's content as last read, which the outboxes send at.
    revisions: std::collections::HashMap<Uuid, u64>,
    contents: Option<LayerContents<R>>,
    applied: Option<R>,
    outboxes: [Outbox<R>; 2],
}

/// An edit the host writes to a layer, made from what that layer holds each time it
/// is applied or sent. A screen's settings are one entry, so an edit made once would
/// put back the rest of the entry as it was then; made again, it keeps a rotation
/// written to the same screen while it was on its way.
pub(crate) type LayerEdit<R> = Rc<dyn Fn(&R) -> Edit>;

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

struct Outbox<R> {
    unwritten: Vec<LayerEdit<R>>,
    sent: Vec<LayerEdit<R>>,
    sent_at: Option<u64>,
}

impl<R> Default for Outbox<R> {
    fn default() -> Self {
        Self {
            unwritten: Vec::new(),
            sent: Vec::new(),
            sent_at: None,
        }
    }
}

impl<R: LocalSettings> Outbox<R> {
    fn queue(&mut self, edit: LayerEdit<R>) {
        self.unwritten.push(edit);
    }

    fn send(&mut self, revision: u64, own: &R) -> Vec<Edit> {
        if self.unwritten.is_empty() {
            return Vec::new();
        }
        let edits = std::mem::take(&mut self.unwritten);
        let (_, made) = made(own, &edits);
        self.sent.extend(edits);
        self.sent_at = Some(revision);
        made
    }

    fn overlay(&mut self, revision: u64, content: R) -> (R, Vec<Edit>) {
        if self.sent.is_empty() {
            return (content, Vec::new());
        }
        let (overlaid, made) = made(&content, &self.sent);
        if overlaid == content {
            self.sent.clear();
            self.sent_at = None;
            return (content, Vec::new());
        }
        let resend = match self.sent_at == Some(revision) {
            true => Vec::new(),
            false => {
                self.sent_at = Some(revision);
                made
            }
        };
        (overlaid, resend)
    }

    fn clear(&mut self) {
        *self = Self::default();
    }
}

/// Applies `edits` in turn, each made from what the ones before it left.
fn made<R: Root>(own: &R, edits: &[LayerEdit<R>]) -> (R, Vec<Edit>) {
    let mut document = Document::new(own);
    let made = edits
        .iter()
        .map(|edit| {
            let made = edit(&document.root());
            document.apply(&made);
            made
        })
        .collect();
    (document.root(), made)
}

impl<R> Default for SettingsSync<R> {
    fn default() -> Self {
        Self {
            blocks: Layers::default(),
            due: true,
            revisions: std::collections::HashMap::new(),
            contents: None,
            applied: None,
            outboxes: [Outbox::default(), Outbox::default()],
        }
    }
}

impl<R: LocalSettings + Layered> SettingsSync<R> {
    pub(crate) fn boot(&mut self, store: &HostStateStore) {
        let name = <R as Layered>::NAME;
        match R::load(store) {
            Ok(Some(content)) => match Document::<R>::decode(&content) {
                Ok(content) => {
                    let settings = content.root();
                    settings.apply();
                    self.applied = Some(settings);
                }
                Err(error) => {
                    eprintln!("be-host: the saved {name} settings are unreadable: {error}")
                }
            },
            Ok(None) => {}
            Err(error) => eprintln!("be-host: the saved {name} settings were not read: {error}"),
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
        self.due = true;
        self.revisions.clear();
        self.contents = None;
    }

    pub(crate) fn layers(&self) -> LayerContents<R> {
        self.contents.clone().unwrap_or_default()
    }

    /// Notes what the host stack changed: a layer block that changed is read again at the next
    /// `sync`.
    pub(crate) fn heard(&mut self, changes: &be_host_stack::Changes) {
        if changes.every_block {
            // The stack started or stopped: revisions count again from one, so what was sent
            // is sent again unless the content the stack opens holds it.
            for outbox in &mut self.outboxes {
                outbox.sent_at = None;
            }
            self.revisions.clear();
        }
        if LAYERS
            .into_iter()
            .filter_map(|layer| self.blocks.block(layer))
            .any(|block| changes.block(block))
        {
            self.due = true;
        }
    }

    pub(crate) fn sync(&mut self, store: &HostStateStore) {
        let blocks = self.blocks;
        if blocks.top().is_none() {
            return;
        }
        let mut read = Vec::with_capacity(2);
        if std::mem::take(&mut self.due) {
            for layer in LAYERS {
                let Some(block) = blocks.block(layer) else {
                    continue;
                };
                be_host_stack::hold(block, R::CONTENT_TYPE);
                // One not loaded yet, or let go (the stack stopped), is read when it arrives,
                // which the stack tells of. Its revisions count again from one then, so nothing
                // is sent at the one read before.
                let Some(content) = be_host_stack::content(block) else {
                    self.revisions.remove(&block);
                    return;
                };
                read.push((layer, block, content));
            }
            for (_, block, content) in &read {
                self.revisions.insert(*block, content.revision);
            }
        }
        for layer in LAYERS {
            let Some(block) = blocks.block(layer) else {
                continue;
            };
            let Some(&revision) = self.revisions.get(&block) else {
                return;
            };
            let own = self.layers().own(layer);
            Self::write(block, self.outboxes[slot(layer)].send(revision, &own));
        }
        if read.is_empty() {
            return;
        }
        let mut contents = LayerContents::default();
        for (layer, block, content) in read {
            let Ok(decoded) = Document::<R>::decode(content.bytes()) else {
                return;
            };
            let (decoded, resend) =
                self.outboxes[slot(layer)].overlay(content.revision, decoded.root());
            Self::write(block, resend);
            contents.set(layer, decoded);
        }
        let settings = contents.layered();
        self.contents = Some(contents);
        self.receive(store, settings);
    }

    fn receive(&mut self, store: &HostStateStore, settings: R) {
        if self.applied.as_ref() == Some(&settings) {
            return;
        }
        if settings.apply() {
            self.save(store, &settings);
        }
        self.applied = Some(settings);
    }

    pub(crate) fn commit(
        &mut self,
        store: &HostStateStore,
        edits: Vec<(SettingsLayer, LayerEdit<R>)>,
    ) {
        let mut contents = self.contents.clone().unwrap_or_default();
        let mut applied = self.applied.clone().unwrap_or_default();
        let mut written = false;
        for (layer, edit) in edits {
            match contents.target(layer) {
                Some(target) => {
                    let (content, _) = made(&contents.own(target), std::slice::from_ref(&edit));
                    contents.set(target, content);
                    self.outboxes[slot(target)].queue(edit);
                    written = true;
                }
                None => {
                    (applied, _) = made(&applied, std::slice::from_ref(&edit));
                    self.outboxes[slot(layer)].queue(edit);
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
                && let Some(&revision) = self.revisions.get(&block)
            {
                let own = self.layers().own(layer);
                Self::write(block, self.outboxes[slot(layer)].send(revision, &own));
            }
        }
    }

    fn write(block: Uuid, edits: Vec<Edit>) {
        for edit in edits {
            be_host_stack::operate(block, Document::<R>::encode_operation(&edit));
        }
    }

    fn save(&self, store: &HostStateStore, settings: &R) {
        if let Err(error) = R::save(store, &Document::new(settings).encode()) {
            eprintln!(
                "be-host: the {} settings were not saved on this device: {error}",
                <R as Layered>::NAME
            );
        }
    }
}

#[cfg(test)]
mod tests;

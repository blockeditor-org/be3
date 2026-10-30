use std::cell::{Cell, RefCell};
use std::rc::Rc;

use be_block::be_model::Document;
use be_block::{BlockContent, Layered, Layers, LiveEdit, Setting, SettingsLayer};
use uuid::Uuid;

use crate::EditorCore;
use crate::root_settings::{RootSetting, SettingsGraph};
use crate::state::{Callbacks, Subscription};

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum SettingsTarget {
    ThisDevice,
    Block(Uuid),
}

impl SettingsTarget {
    pub fn choose(page: bool, block: Uuid, layers: &Layers) -> Self {
        match page && layers.every_device != Some(block) {
            true => Self::Block(block),
            false => Self::ThisDevice,
        }
    }
}

pub struct SettingsLayers<R> {
    core: EditorCore,
    blocks: RefCell<RootSetting<Document<R>>>,
}

impl<R> SettingsLayers<R>
where
    R: Layered + 'static,
    Document<R>: LiveEdit<Op = be_block::Edit> + BlockContent + Clone + Default,
{
    pub fn new(core: EditorCore) -> Self {
        Self {
            core,
            blocks: RefCell::default(),
        }
    }

    pub fn layer_blocks(&self, graph: &dyn SettingsGraph) -> Layers {
        let client = self.core.host().client_id();
        self.blocks.borrow().layer_blocks(graph, client)
    }

    pub fn ensure(&self, layers: Layers, layer: SettingsLayer) -> Option<Uuid> {
        layers
            .block(layer)
            .or_else(|| self.create(layer, Document::new(&R::default())))
    }

    pub fn create(&self, layer: SettingsLayer, content: Document<R>) -> Option<Uuid> {
        let client = self.core.host().client_id();
        let mut blocks = self.blocks.take();
        let block = blocks.create_layer(&self.core, client, layer, content);
        self.blocks.replace(blocks);
        block
    }

    pub fn write<V: Clone>(
        &self,
        layers: Layers,
        layer: SettingsLayer,
        setting: &Setting<R, V>,
        value: Option<V>,
    ) {
        if let Some(block) = layers.block(layer) {
            self.write_block(block, setting, value);
            return;
        }
        let mut content = Document::new(&R::default());
        content.apply(&setting.set(&R::default(), value.clone()));
        if self.create(layer, content).is_none()
            && let Some(block) = layers.top()
        {
            self.write_block(block, setting, value);
        }
    }

    pub fn write_block<V: Clone>(&self, block: Uuid, setting: &Setting<R, V>, value: Option<V>) {
        let content = self.core.content_of::<Document<R>>(block);
        let own = content.read(|content| content.root()).unwrap_or_default();
        content.operate(setting.set(&own, value));
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SettingsAnswer {
    KeepOnThisDevice,
    Undo,
}

struct SettingsChange {
    key: String,
    name: String,
    undo: Rc<dyn Fn()>,
    keep: Rc<dyn Fn()>,
}

#[derive(Default)]
pub struct SettingsChangeLog {
    changes: RefCell<Vec<SettingsChange>>,
    round: Cell<u64>,
    changed: Callbacks<(), dyn Fn()>,
}

impl SettingsChangeLog {
    pub fn on_change(&self, run: impl Fn() + 'static) -> Subscription {
        self.changed.add((), Rc::new(run))
    }

    fn changed(&self) {
        for run in self.changed.all() {
            run();
        }
    }

    pub fn noted(&self, key: &str) -> bool {
        self.changes
            .borrow()
            .iter()
            .any(|changed| changed.key == key)
    }

    pub fn note(
        &self,
        key: &str,
        name: &str,
        undo: impl Fn() + 'static,
        keep: impl Fn() + 'static,
    ) {
        if self.noted(key) {
            return;
        }
        self.changes.borrow_mut().push(SettingsChange {
            key: key.to_owned(),
            name: name.to_owned(),
            undo: Rc::new(undo),
            keep: Rc::new(keep),
        });
        self.changed();
    }

    pub fn round(&self) -> u64 {
        self.round.get()
    }

    pub fn message(&self) -> Option<String> {
        let mut names: Vec<String> = Vec::new();
        for changed in self.changes.borrow().iter() {
            if !names.contains(&changed.name) {
                names.push(changed.name.clone());
            }
        }
        changes_message(&names)
    }

    pub fn dismiss(&self) {
        self.changes.borrow_mut().clear();
        self.round.set(self.round.get() + 1);
        self.changed();
    }

    pub fn answer(&self, answer: SettingsAnswer) {
        let answered = std::mem::take(&mut *self.changes.borrow_mut());
        self.round.set(self.round.get() + 1);
        self.changed();
        for changed in answered {
            match answer {
                SettingsAnswer::KeepOnThisDevice => (changed.keep)(),
                SettingsAnswer::Undo => (changed.undo)(),
            }
        }
    }
}

fn changes_message(names: &[String]) -> Option<String> {
    let lower = |name: &String| {
        let mut chars = name.chars();
        chars.next().map_or_else(String::new, |first| {
            first.to_lowercase().chain(chars).collect()
        })
    };
    let listed = match names {
        [] => return None,
        [one] => return Some(format!("{one} now applies on your devices")),
        [first, second] => format!("{first} and {}", lower(second)),
        [first, second, rest @ ..] => {
            format!("{first}, {} and {} more", lower(second), rest.len())
        }
    };
    Some(format!("{listed} now apply on your devices"))
}

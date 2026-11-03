use std::cell::{Cell, RefCell};
use std::rc::Rc;

use be_block::be_block_model::Document;
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

    pub fn ensure(&self, graph: &dyn SettingsGraph, layer: SettingsLayer) -> Option<Uuid> {
        self.layer_blocks(graph)
            .block(layer)
            .or_else(|| self.create(graph, layer, Document::new(&R::default())))
    }

    pub fn create(
        &self,
        graph: &dyn SettingsGraph,
        layer: SettingsLayer,
        content: Document<R>,
    ) -> Option<Uuid> {
        let client = self.core.host().client_id();
        let mut blocks = self.blocks.take();
        let block = blocks.create_layer(graph, client, layer, content);
        self.blocks.replace(blocks);
        block
    }

    pub fn write<V: Clone + 'static>(
        &self,
        graph: &dyn SettingsGraph,
        layer: SettingsLayer,
        setting: &Setting<R, V>,
        value: Option<V>,
    ) {
        let setting = setting.clone();
        let edit: KeptEdit<R> = Rc::new(move |own: &R| setting.set(own, value.clone()));
        self.write_all(graph, layer, &[edit]);
    }

    pub fn write_all(
        &self,
        graph: &dyn SettingsGraph,
        layer: SettingsLayer,
        edits: &[KeptEdit<R>],
    ) {
        let layers = self.layer_blocks(graph);
        if let Some(block) = layers.block(layer) {
            self.edit_block(block, edits);
            return;
        }
        let mut content = Document::new(&R::default());
        for edit in edits {
            let made = edit(&content.root());
            content.apply(&made);
        }
        if self.create(graph, layer, content).is_none()
            && let Some(block) = layers.top()
        {
            self.edit_block(block, edits);
        }
    }

    fn edit_block(&self, block: Uuid, edits: &[KeptEdit<R>]) {
        let content = self.core.content_of::<Document<R>>(block);
        for edit in edits {
            let own = content.read(|content| content.root()).unwrap_or_default();
            content.operate(edit(&own));
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

pub type KeptEdit<R> = Rc<dyn Fn(&R) -> be_block::Edit>;

struct SettingsChange<R> {
    key: String,
    name: String,
    undo: Rc<dyn Fn()>,
    here: KeptEdit<R>,
}

pub struct SettingsChangeLog<R> {
    changes: RefCell<Vec<SettingsChange<R>>>,
    round: Cell<u64>,
    changed: Callbacks<(), dyn Fn()>,
}

impl<R> Default for SettingsChangeLog<R> {
    fn default() -> Self {
        Self {
            changes: RefCell::default(),
            round: Cell::default(),
            changed: Callbacks::default(),
        }
    }
}

impl<R> SettingsChangeLog<R> {
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
        here: impl Fn(&R) -> be_block::Edit + 'static,
    ) {
        if self.noted(key) {
            return;
        }
        self.changes.borrow_mut().push(SettingsChange {
            key: key.to_owned(),
            name: name.to_owned(),
            undo: Rc::new(undo),
            here: Rc::new(here),
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

    pub fn answer(&self, answer: SettingsAnswer, keep_here: impl FnOnce(&[KeptEdit<R>])) {
        let answered = std::mem::take(&mut *self.changes.borrow_mut());
        self.round.set(self.round.get() + 1);
        self.changed();
        if answer == SettingsAnswer::KeepOnThisDevice {
            let kept: Vec<KeptEdit<R>> = answered
                .iter()
                .map(|changed| Rc::clone(&changed.here))
                .collect();
            keep_here(&kept);
        }
        for changed in answered {
            (changed.undo)();
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

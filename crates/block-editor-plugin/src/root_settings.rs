use be_block::settings::{ActivationCondition, Settings};
use be_block::{BlockContent, Edit, Root, SettingsContent};
use uuid::Uuid;

use crate::BlockParent;

pub trait SettingsGraph {
    fn roots(&self) -> Option<Vec<(Uuid, Uuid)>>;
    fn settings(&self, block: Uuid) -> Option<Settings>;
    fn create(&self, block_type: Uuid, content: Vec<u8>, parent: BlockParent) -> Uuid;
    fn edit_settings(&self, block: Uuid, edit: Edit);
}

#[derive(Default)]
pub struct RootSettings {
    block: Option<Uuid>,
}

impl RootSettings {
    pub fn find(&mut self, graph: &dyn SettingsGraph) -> Option<Uuid> {
        if self.block.is_none() {
            self.block = graph
                .roots()?
                .into_iter()
                .find(|(_, block_type)| *block_type == Settings::CONTENT_TYPE)
                .map(|(id, _)| id);
        }
        self.block
    }

    pub fn ensure(&mut self, graph: &dyn SettingsGraph) -> Option<Uuid> {
        if self.find(graph).is_none() && graph.roots().is_some() {
            self.block = Some(graph.create(
                SettingsContent::CONTENT_TYPE,
                SettingsContent::default().encode(),
                BlockParent::Root,
            ));
        }
        self.block
    }
}

pub struct RootSetting<C> {
    settings: RootSettings,
    block: Option<Uuid>,
    content: std::marker::PhantomData<C>,
}

impl<C> Default for RootSetting<C> {
    fn default() -> Self {
        Self {
            settings: RootSettings::default(),
            block: None,
            content: std::marker::PhantomData,
        }
    }
}

impl<C: BlockContent + Default> RootSetting<C> {
    pub fn block(&self) -> Option<Uuid> {
        self.block
    }

    pub fn find(&mut self, graph: &dyn SettingsGraph, client: Uuid) -> Option<Uuid> {
        if self.block.is_none() {
            let settings = graph.settings(self.settings.find(graph)?)?;
            self.block = settings.resolve(C::CONTENT_TYPE, client);
        }
        self.block
    }

    pub fn ensure(&mut self, graph: &dyn SettingsGraph, client: Uuid) -> Option<Uuid> {
        if self.find(graph, client).is_some() {
            return self.block;
        }
        let settings = self.settings.ensure(graph)?;
        graph.settings(settings)?;
        let block = graph.create(
            C::CONTENT_TYPE,
            C::default().encode(),
            BlockParent::Block(settings),
        );
        graph.edit_settings(
            settings,
            Settings::set_entry(C::CONTENT_TYPE, ActivationCondition::Fallback, block),
        );
        self.block = Some(block);
        self.block
    }
}

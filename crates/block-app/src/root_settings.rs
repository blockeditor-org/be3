use be_block::settings::{ActivationCondition, Settings};
use be_block::{BlockContent, BlockMetadata, LiveEdit, Root, SettingsContent};
use be_graph::BlockParent;
use uuid::Uuid;

use crate::be;

pub(crate) fn settings(block: Uuid) -> Option<Settings> {
    be::hold(block, SettingsContent::CONTENT_TYPE);
    let content = be::content(block)?;
    SettingsContent::decode(&content.bytes)
        .ok()
        .map(|document| document.root().clone())
}

#[derive(Default)]
pub(crate) struct RootSettings {
    block: Option<Uuid>,
}

impl RootSettings {
    pub(crate) fn find(&mut self) -> Option<Uuid> {
        if self.block.is_none() {
            self.block = be::query(be::Query::Roots)
                .into_iter()
                .find(|node| node.content_type == Settings::CONTENT_TYPE)
                .map(|node| node.id);
        }
        self.block
    }

    pub(crate) fn ensure(&mut self) -> Option<Uuid> {
        if self.find().is_none() && be::graph_loaded() {
            let block = Uuid::new_v4();
            be::create(
                block,
                SettingsContent::CONTENT_TYPE,
                BlockParent::Root,
                BlockMetadata::default(),
                Some(SettingsContent::default().encode()),
            );
            self.block = Some(block);
        }
        self.block
    }
}

pub(crate) struct RootSetting<C> {
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
    pub(crate) fn find(&mut self, client: Uuid) -> Option<Uuid> {
        if self.block.is_none() {
            self.block = settings(self.settings.find()?)?.resolve(C::CONTENT_TYPE, client);
        }
        self.block
    }

    pub(crate) fn ensure(&mut self, client: Uuid) -> Option<Uuid> {
        if self.find(client).is_some() {
            return self.block;
        }
        let settings_block = self.settings.ensure()?;
        settings(settings_block)?;
        let block = Uuid::new_v4();
        be::create(
            block,
            C::CONTENT_TYPE,
            BlockParent::Block(settings_block),
            BlockMetadata::default(),
            Some(C::default().encode()),
        );
        be::operate_from(
            settings_block,
            be::next_origin(),
            SettingsContent::encode_operation(&Settings::set_entry(
                C::CONTENT_TYPE,
                ActivationCondition::Fallback,
                block,
            )),
        );
        self.block = Some(block);
        self.block
    }
}

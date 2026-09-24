use be_block::settings::{ActivationCondition, Settings};
use be_block::{BlockContent, Edit};
use block::{Block, BlockParent, BlockReferenceList};
use uuid::Uuid;

use crate::{BlockClient, BlockHandle, ReferenceList};

pub trait SettingsStore {
    fn settings(&self, block: Uuid) -> Option<Settings>;
    fn seed(&self, block: Uuid, content_type: Uuid, bytes: Vec<u8>);
    fn edit_settings(&self, block: Uuid, edit: Edit);
}

pub struct RootSettings {
    roots: ReferenceList,
    block: Option<Uuid>,
}

impl RootSettings {
    pub fn new(client: &BlockClient) -> Self {
        Self {
            roots: client.watch_references(BlockReferenceList::Roots),
            block: None,
        }
    }

    pub fn find(&mut self) -> Option<Uuid> {
        if self.block.is_none() {
            let found = self.roots.read().into_iter().find(|reference| {
                reference.block_type == crate::blocks::settings::Settings::TYPE_ID
            })?;
            self.block = Some(found.id);
        }
        self.block
    }

    pub fn ensure(&mut self, client: &BlockClient, store: &dyn SettingsStore) -> Option<Uuid> {
        if self.find().is_none() && self.roots.is_loaded() {
            let settings = client.create_block(crate::blocks::settings::Settings);
            store.seed(
                settings.id(),
                be_block::SettingsContent::CONTENT_TYPE,
                be_block::SettingsContent::default().encode(),
            );
            settings.set_parent(BlockParent::Root);
            self.block = Some(settings.id());
        }
        self.block
    }
}

pub struct RootSetting<T: Block, C> {
    settings: RootSettings,
    block: Option<BlockHandle<T>>,
    content: std::marker::PhantomData<C>,
}

impl<T: Block + Default, C: BlockContent + Default> RootSetting<T, C> {
    pub fn new(client: &BlockClient) -> Self {
        Self {
            settings: RootSettings::new(client),
            block: None,
            content: std::marker::PhantomData,
        }
    }

    pub fn block(&self) -> Option<&BlockHandle<T>> {
        self.block.as_ref()
    }

    pub fn find(
        &mut self,
        client: &BlockClient,
        store: &dyn SettingsStore,
        client_id: Uuid,
    ) -> Option<&BlockHandle<T>> {
        if self.block.is_none() {
            let settings = store.settings(self.settings.find()?)?;
            let id = settings.resolve(T::TYPE_ID, client_id)?;
            self.block = Some(client.get_block::<T>(id));
        }
        self.block.as_ref()
    }

    pub fn ensure(
        &mut self,
        client: &BlockClient,
        store: &dyn SettingsStore,
        client_id: Uuid,
    ) -> Option<&BlockHandle<T>> {
        if self.find(client, store, client_id).is_some() {
            return self.block.as_ref();
        }
        let settings = self.settings.ensure(client, store)?;
        store.settings(settings)?;
        let block = client.create_block(T::default());
        store.seed(block.id(), C::CONTENT_TYPE, C::default().encode());
        store.edit_settings(
            settings,
            Settings::set_entry(T::TYPE_ID, ActivationCondition::Fallback, block.id()),
        );
        block.set_parent(BlockParent::Uuid(settings));
        self.block = Some(block);
        self.block.as_ref()
    }
}

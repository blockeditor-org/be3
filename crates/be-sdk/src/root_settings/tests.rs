use super::*;

use std::cell::RefCell;

mod a_kind_made_for_an_account_with_no_settings_lands_at_once;
mod an_owner_only_kind_makes_its_block_owner_only;

#[derive(Default)]
pub(super) struct Graph {
    pub(super) account: Uuid,
    pub(super) settings: RefCell<Option<Uuid>>,
    pub(super) created: RefCell<Vec<(Uuid, Uuid, bool)>>,
    pub(super) contents: RefCell<Vec<(Uuid, Vec<u8>)>>,
    /// Whether a block's content can be read back, which one just made can't yet.
    pub(super) loaded: bool,
}

impl SettingsGraph for Graph {
    fn roots(&self) -> Option<Vec<(Uuid, Uuid, Uuid)>> {
        Some(
            self.settings
                .borrow()
                .iter()
                .map(|block| (*block, Settings::CONTENT_TYPE, self.account))
                .collect(),
        )
    }

    fn account(&self) -> Uuid {
        self.account
    }

    fn settings(&self, block: Uuid) -> Option<Settings> {
        (self.loaded && *self.settings.borrow() == Some(block)).then(Settings::default)
    }

    fn settings_profile(&self, _: Uuid) -> Option<SettingsProfile> {
        None
    }

    fn create(
        &self,
        block: Uuid,
        block_type: Uuid,
        content: Vec<u8>,
        parent: BlockParent,
        owner_only: bool,
    ) -> Uuid {
        self.contents.borrow_mut().push((block, content));
        if parent == BlockParent::Root {
            *self.settings.borrow_mut() = Some(block);
        }
        self.created
            .borrow_mut()
            .push((block, block_type, owner_only));
        block
    }

    fn edit_settings(&self, _: Uuid, _: Edit) {}

    fn edit_settings_profile(&self, _: Uuid, _: Edit) {}
}

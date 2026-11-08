use super::*;

use std::cell::RefCell;

use be_block::SettingsProfileContent;

#[derive(Default)]
struct Graph {
    account: Uuid,
    settings: RefCell<Option<Uuid>>,
    created: RefCell<Vec<(Uuid, Uuid, bool)>>,
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
        (*self.settings.borrow() == Some(block)).then(Settings::default)
    }

    fn settings_profile(&self, _: Uuid) -> Option<SettingsProfile> {
        None
    }

    fn create(&self, block_type: Uuid, _: Vec<u8>, parent: BlockParent, owner_only: bool) -> Uuid {
        let block = Uuid::now_v7();
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

#[test]
fn an_owner_only_kind_makes_its_block_owner_only() {
    let graph = Graph::default();
    let kind = RootSetting::<SettingsProfileContent>::owner_only()
        .ensure(&graph, Uuid::nil())
        .unwrap();
    let ordinary = RootSetting::<SettingsProfileContent>::default()
        .ensure(&graph, Uuid::nil())
        .unwrap();
    let created = graph.created.borrow().clone();
    let owner_only = |block: Uuid| {
        created
            .iter()
            .find(|(id, _, _)| *id == block)
            .map(|(_, _, owner_only)| *owner_only)
    };
    assert_eq!(owner_only(kind), Some(true));
    assert_eq!(owner_only(ordinary), Some(false));
    let settings = graph.settings.borrow().unwrap();
    assert_eq!(
        owner_only(settings),
        Some(false),
        "the account's Settings block stays readable by administrators"
    );
}

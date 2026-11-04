use super::*;
use crate::root::StorageRoot;
use be_sdk_beui::{AccountStorage, StorageUsageWatch};

const GB: u64 = 1_000_000_000;

#[test]
fn storage_shows_the_quota_used_and_goes_into_the_blocks_that_use_it() {
    let mut storage = editor_as::<StorageRoot>(Uuid::nil());
    storage.set_size(Vec2::new(900.0, 700.0));
    assert!(
        storage.shown("settings.storage.unknown"),
        "until the server says, the page says it does not know"
    );
    storage.set_watched::<StorageUsageWatch>(&Some(AccountStorage {
        held: 7 * GB,
        pending: GB / 2,
        quota: Some(10 * GB),
    }));
    let (work, personal, photos, notes) = (
        Uuid::from_u128(0xa1),
        Uuid::from_u128(0xa2),
        Uuid::from_u128(0xa3),
        Uuid::from_u128(0xa4),
    );
    let store = storage.store();
    for (id, parent, name, size, tree_size) in [
        (work, BlockParent::Root, "Work", GB / 10, 6 * GB),
        (personal, BlockParent::Root, "Personal", GB, GB),
        (photos, BlockParent::Block(work), "Photos", GB / 5, 4 * GB),
        (
            notes,
            BlockParent::Block(work),
            "Notes",
            2 * GB - GB / 10,
            2 * GB - GB / 10,
        ),
    ] {
        let mut info = BlockInfo::new(id, SettingsContent::CONTENT_TYPE, parent);
        info.set_name(name);
        info.size = size;
        info.tree_size = tree_size;
        if id == work {
            info.references = vec![photos, notes];
        }
        store.add_block(info);
    }
    storage.run();
    assert!(!storage.shown("settings.storage.unknown"));
    assert_eq!(
        storage.label("settings.storage.used"),
        "7.5 GB of 10 GB used"
    );
    assert!(storage.shown(&format!("settings.storage.block.{personal}")));
    assert!(
        !storage.shown(&format!("settings.storage.block.{photos}")),
        "a block's children are listed only once it is gone into"
    );

    storage.click(&format!("settings.storage.block.{work}"));
    storage.run();
    for inside in [photos, notes, work] {
        assert!(
            storage.shown(&format!("settings.storage.block.{inside}")),
            "Work shows its children and its own bytes"
        );
    }
    assert!(!storage.shown(&format!("settings.storage.block.{personal}")));
    storage.snapshot("storage_inside_a_workspace");

    storage.click("settings.storage.up");
    storage.run();
    assert!(storage.shown(&format!("settings.storage.block.{personal}")));
}

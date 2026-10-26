use block_editor_beui::be_block::be_model::BlockRef;
use block_editor_beui::be_block::{
    BlockContent, PREFIXES, PrefixRegistry, PrefixRegistryContent, WORKSPACE_TYPE,
};
use block_editor_beui::{BlockInfo, BlockParent};

use super::*;

#[test]
fn a_prefix_is_listed_in_the_workspaces_prefixes_and_a_shared_one_says_so() {
    let mut fixture = editor(&[("Title", DatabaseFieldType::String)]);
    let view = fixture.harness.block_id().expect("the view is a block");
    let [workspace, registry, bugs] = [1, 2, 3].map(|id| Uuid::from_u128(0xdb_0000 + id));
    let store = fixture.harness.store();
    let mut held = BlockInfo::new(workspace, WORKSPACE_TYPE, BlockParent::Root);
    held.metadata.insert(&PREFIXES, &BlockRef(registry));
    store.add_block(held);
    store.add_block(BlockInfo::new(
        registry,
        PrefixRegistryContent::CONTENT_TYPE,
        BlockParent::Block(workspace),
    ));
    let mut other = BlockInfo::new(
        bugs,
        DatabaseContent::CONTENT_TYPE,
        BlockParent::Block(workspace),
    );
    other.set_name("Bugs".to_owned());
    store.add_block(other);
    store.add_block(BlockInfo::new(
        view,
        DatabaseViewContent::CONTENT_TYPE,
        BlockParent::Block(workspace),
    ));
    let mut listed = PrefixRegistryContent::default();
    listed.apply(&PrefixRegistry::register(bugs, Some("BE")));
    fixture.harness.hold(Some(registry), listed);
    fixture.settle();

    fixture.harness.click("database-view.display");
    fixture.settle();
    fixture.harness.click("database-view.prefix");
    fixture.run();
    fixture.harness.type_text("be");
    fixture.settle();

    let registered = fixture
        .harness
        .content::<PrefixRegistryContent>(Some(registry))
        .root();
    let mut both = registered.databases_with("BE");
    both.sort_unstable();
    let mut wanted = vec![bugs, fixture.database];
    wanted.sort_unstable();
    assert_eq!(both, wanted, "the database lists itself beside Bugs");
    assert!(
        fixture.harness.shown("database-view.prefix-shared"),
        "the display options say the prefix is shared"
    );
    fixture
        .harness
        .snapshot("a_prefix_is_listed_in_the_workspaces_prefixes_and_a_shared_one_says_so");
}

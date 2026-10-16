use block_editor_beui::{AccessLevel, BlockInfo, BlockParent, BlockQuery};

use super::*;

#[test]
fn a_phone_lists_what_is_shared_with_it() {
    let mut fixture = phone();
    let plan = Uuid::from_u128(5);
    my_files(&mut fixture, Vec::new());
    let mut given = BlockInfo::new(
        plan,
        Uuid::from_u128(7),
        BlockParent::Block(Uuid::from_u128(6)),
    );
    given.set_intrinsic_name("Plan");
    given.author = Some(Uuid::from_u128(9));
    given.access = AccessLevel::View;
    fixture.host.set_blocks(BlockQuery::Shared, vec![given]);
    fixture.settle();
    assert!(!fixture.test.shown(&format!("file-tree.{plan}.open")));

    fixture.test.click("file-tree.shared");
    fixture.settle();
    assert!(fixture.says("Shared with me"));
    assert!(fixture.test.shown(&format!("file-tree.{plan}.open")));
    fixture
        .test
        .snapshot("a_phone_lists_what_is_shared_with_it");

    fixture.test.click(&format!("file-tree.{plan}.open"));
    fixture.settle();
    assert_eq!(
        fixture.opened(),
        vec![plan],
        "tapping a shared file opens it"
    );
}

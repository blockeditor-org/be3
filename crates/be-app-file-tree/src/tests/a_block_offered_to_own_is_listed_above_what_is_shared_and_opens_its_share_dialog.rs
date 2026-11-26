use be_sdk_beui::{AccessLevel, BlockCommand, BlockInfo, BlockParent, BlockQuery};

use super::*;

#[test]
fn a_block_offered_to_own_is_listed_above_what_is_shared_and_opens_its_share_dialog() {
    let mut fixture = editor();
    let text_type = Uuid::from_u128(7);
    let (plan, notes, elsewhere) = (Uuid::from_u128(5), Uuid::from_u128(3), Uuid::from_u128(6));
    my_files(&mut fixture, Vec::new());
    let mut given = BlockInfo::new(notes, text_type, BlockParent::Block(elsewhere));
    given.set_intrinsic_name("Notes");
    given.author = Some(Uuid::from_u128(9));
    given.access = AccessLevel::View;
    fixture.host.set_blocks(BlockQuery::Shared, vec![given]);
    fixture.host.set_blocks(BlockQuery::OwnerOffers, Vec::new());
    fixture.settle();
    assert!(
        !fixture.test.shown("file-tree.offered.row"),
        "with no offer standing there is no section for one"
    );

    let mut offered = BlockInfo::new(plan, text_type, BlockParent::Block(elsewhere));
    offered.set_intrinsic_name("Plan");
    offered.author = Some(Uuid::from_u128(9));
    offered.access = AccessLevel::Edit;
    fixture
        .host
        .set_blocks(BlockQuery::OwnerOffers, vec![offered]);
    fixture.settle();
    let top = |fixture: &Fixture, id: &str| fixture.test.rect_of(id).top();
    let order = [
        top(&fixture, &format!("file-tree.{MY_FILES}.row")),
        top(&fixture, "file-tree.offered.row"),
        top(&fixture, &format!("file-tree.offer.{plan}.row")),
        top(&fixture, "file-tree.shared.row"),
        top(&fixture, &format!("file-tree.{notes}.row")),
    ];
    assert!(
        order.windows(2).all(|pair| pair[0] < pair[1]),
        "Offered to you sits below the workspaces and above what is shared: {order:?}"
    );
    assert!(fixture.says("Offered to you"));
    fixture.test.snapshot(
        "a_block_offered_to_own_is_listed_above_what_is_shared_and_opens_its_share_dialog",
    );

    fixture.test.click(format!("file-tree.offer.{plan}.row"));
    fixture.settle();
    assert_eq!(
        fixture.test.take_block_commands(),
        vec![(plan, BlockCommand::Share)],
        "an offered block opens its share dialog, where the offer is accepted"
    );
    assert!(fixture.opened().is_empty(), "nor does it open the block");

    fixture.host.set_blocks(BlockQuery::OwnerOffers, Vec::new());
    fixture.settle();
    assert!(
        !fixture.test.shown("file-tree.offered.row"),
        "once accepted or withdrawn the section goes"
    );
}

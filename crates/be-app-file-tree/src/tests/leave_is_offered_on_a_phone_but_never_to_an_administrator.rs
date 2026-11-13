use be_sdk_beui::{
    AccessLevel, AdministratorWatch, BlockCommand, BlockInfo, BlockParent, BlockQuery,
};

use super::*;

#[test]
fn leave_is_offered_on_a_phone_but_never_to_an_administrator() {
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

    let more = format!("file-tree.{plan}.more");
    fixture.test.click(&more);
    fixture.settle();
    fixture.test.settle();
    fixture
        .test
        .snapshot("a_row_menu_with_room_on_neither_side_covers_its_row_to_fit_the_phone");
    fixture.click_item("Leave");
    assert!(
        fixture
            .test
            .take_block_commands()
            .contains(&(plan, BlockCommand::Leave)),
        "leaving what was shared asks the host to take this account off it"
    );

    fixture.test.set_watched::<AdministratorWatch>(&true);
    fixture.settle();
    fixture.test.click(&more);
    fixture.settle();
    assert!(fixture.offers("Delete"), "the row's menu is open");
    assert!(
        !fixture.offers("Leave"),
        "an administrator keeps every block whatever it leaves, so it is not offered"
    );
}

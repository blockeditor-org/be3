use super::*;
use be_block_protocol::Listing;

#[test]
fn a_listed_block_this_device_may_no_longer_know_of_is_left_out_offline() {
    let refs = scoped(&refs::RefsDb::memory());
    let folder = BlockParent::Block(Uuid::from_u128(10));
    refs.listed(
        Listing::Children(folder),
        &[placed(20, folder, 2), placed(21, folder, 3)],
        5,
    )
    .unwrap();
    refs.lose_access(Uuid::from_u128(20)).unwrap();
    assert_eq!(
        offline_ids(&refs, Listing::Children(folder)),
        Some(vec![Uuid::from_u128(21)])
    );
}

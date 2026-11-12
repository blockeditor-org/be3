use super::*;
use be_block_protocol::Listing;

#[test]
fn a_block_moved_out_of_a_listed_folder_is_listed_where_it_went_offline() {
    let refs = scoped(&refs::RefsDb::memory());
    let folder = BlockParent::Block(Uuid::from_u128(10));
    let other = BlockParent::Block(Uuid::from_u128(11));
    refs.listed(
        Listing::Children(folder),
        &[placed(20, folder, 2), placed(21, folder, 3)],
        5,
    )
    .unwrap();
    refs.listed(Listing::Children(other), &[], 5).unwrap();
    let moved = placed(20, other, 6);
    refs.set_listing(Listing::Block(moved.id), &[moved]).unwrap();
    assert_eq!(
        offline_ids(&refs, Listing::Children(folder)),
        Some(vec![Uuid::from_u128(21)])
    );
    assert_eq!(
        offline_ids(&refs, Listing::Children(other)),
        Some(vec![Uuid::from_u128(20)])
    );
}

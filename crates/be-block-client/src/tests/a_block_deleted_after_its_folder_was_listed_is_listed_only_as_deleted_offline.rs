use super::*;
use be_block_protocol::Listing;

#[test]
fn a_block_deleted_after_its_folder_was_listed_is_listed_only_as_deleted_offline() {
    let refs = scoped(&refs::RefsDb::memory());
    let folder = BlockParent::Block(Uuid::from_u128(10));
    refs.listed(Listing::Children(folder), &[placed(20, folder, 2)], 5)
        .unwrap();
    refs.listed(Listing::Deleted, &[], 5).unwrap();
    let deleted = placed(20, BlockParent::Detached, 6);
    refs.set_listing(Listing::Block(deleted.id), &[deleted])
        .unwrap();
    assert_eq!(
        offline_ids(&refs, Listing::Children(folder)),
        Some(Vec::new())
    );
    assert_eq!(
        offline_ids(&refs, Listing::Deleted),
        Some(vec![Uuid::from_u128(20)])
    );
}

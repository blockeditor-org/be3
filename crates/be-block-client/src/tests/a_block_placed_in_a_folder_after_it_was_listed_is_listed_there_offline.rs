use super::*;
use be_block_protocol::Listing;

#[test]
fn a_block_placed_in_a_folder_after_it_was_listed_is_listed_there_offline() {
    let refs = scoped(&refs::RefsDb::memory());
    let folder = BlockParent::Block(Uuid::from_u128(10));
    let elsewhere = BlockParent::Block(Uuid::from_u128(11));
    refs.listed(Listing::Children(folder), &[placed(20, folder, 4)], 5)
        .unwrap();
    // Told of after the listing: one moved in, one made there, and one the listing would have
    // named had it been there when it was read.
    for summary in [
        placed(21, elsewhere, 3),
        placed(21, folder, 6),
        placed(22, folder, 7),
        placed(23, folder, 5),
    ] {
        refs.set_listing(Listing::Block(summary.id), &[summary])
            .unwrap();
    }
    assert_eq!(
        offline_ids(&refs, Listing::Children(folder)),
        Some(vec![
            Uuid::from_u128(20),
            Uuid::from_u128(21),
            Uuid::from_u128(22)
        ]),
        "a summary as old as the listing that it left out was elsewhere when it was read",
    );
}

use super::*;
use be_block_protocol::Listing;

/// A summary newer than the first listing, which the second one leaves out, was already gone
/// by the second: the listing is placed as of when it was last read, across a restart too.
#[test]
fn a_folder_listed_again_answers_offline_as_of_the_newer_listing() {
    let db = refs::RefsDb::memory();
    let refs = scoped(&db);
    let folder = BlockParent::Block(Uuid::from_u128(10));
    refs.listed(Listing::Children(folder), &[placed(20, folder, 2)], 5)
        .unwrap();
    let made = placed(21, folder, 6);
    refs.set_listing(Listing::Block(made.id), &[made]).unwrap();
    assert_eq!(
        offline_ids(&refs, Listing::Children(folder)),
        Some(vec![Uuid::from_u128(20), Uuid::from_u128(21)])
    );

    refs.listed(Listing::Children(folder), &[placed(20, folder, 2)], 8)
        .unwrap();
    assert_eq!(
        offline_ids(&refs, Listing::Children(folder)),
        Some(vec![Uuid::from_u128(20)])
    );
    drop(refs);
    assert_eq!(
        offline_ids(&scoped(&db), Listing::Children(folder)),
        Some(vec![Uuid::from_u128(20)]),
        "the revision a listing was read at is kept with it",
    );
}

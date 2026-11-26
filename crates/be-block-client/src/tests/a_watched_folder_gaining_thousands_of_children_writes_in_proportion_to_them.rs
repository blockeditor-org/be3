use super::*;
use be_block_protocol::Listing;

/// A watcher of a folder reads its listing again as each child joins it, and a device that
/// makes children offline adds each to the listing it keeps: either way what this device
/// writes for a child is its own rows, not the listing's every block again, so thousands of
/// children write bytes linear in them.
#[test]
fn a_watched_folder_gaining_thousands_of_children_writes_in_proportion_to_them() {
    const CHILDREN: u128 = 2000;
    let db = refs::RefsDb::memory();
    let refs = scoped(&db);
    let folder = BlockParent::Block(Uuid::from_u128(10));
    let listing = Listing::Children(folder);
    let mut children = Vec::new();
    let mut written = Vec::new();
    for child in 0..CHILDREN {
        // Ids that land all over the listing's order, not only at its end.
        let id = (child * 0x9e37_79b9_7f4a_7c15) % (1 << 64) + 100;
        let sequence = child as u64 + 1;
        children.push(placed(id, folder, sequence));
        let before = db.written();
        refs.listed(listing, &children, sequence).unwrap();
        written.push(db.written() - before);
    }
    let first: u64 = written[..written.len() / 2].iter().sum();
    let last: u64 = written[written.len() / 2..].iter().sum();
    assert!(
        last <= first + first / 10,
        "the second half of the children wrote {last} bytes, the first {first}",
    );
    let most = *written.iter().max().unwrap();
    assert!(most < 1024, "one child joining wrote {most} bytes");

    // Made here offline, each added to the listing kept of the folder.
    let before = db.written();
    for child in 0..CHILDREN {
        let made = placed((child + 1) << 80, folder, CHILDREN as u64 + 1 + child as u64);
        let one = db.written();
        refs.add_to_listing(listing, &made).unwrap();
        assert!(db.written() - one < 1024, "adding one child wrote too much");
    }
    let added = db.written() - before;
    assert!(
        added < CHILDREN as u64 * 1024,
        "adding {CHILDREN} children wrote {added} bytes",
    );

    drop(refs);
    let held = scoped(&db).listing(&listing).unwrap();
    assert_eq!(held.len(), 2 * CHILDREN as usize, "every child is kept");
    assert!(
        held.windows(2).all(|pair| pair[0].id < pair[1].id),
        "a listing reads back in the order of its blocks' ids, as the server answers",
    );
}

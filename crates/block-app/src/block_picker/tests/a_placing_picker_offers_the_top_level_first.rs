use be_graph::BlockParent;

use super::*;

#[test]
fn a_placing_picker_offers_the_top_level_first() {
    let registry = registry();
    let folder = Uuid::new_v4();

    let offered = places(&registry, BlockParent::Root, &HashSet::new());
    assert_eq!(
        offered.first().map(|place| place.parent),
        Some(BlockParent::Root),
        "the top level is always a place a new block can go"
    );
    assert_eq!(offered[0].name, "Top level");

    let offered = places(&registry, BlockParent::Block(folder), &HashSet::from([folder]));
    assert!(
        offered.iter().all(|place| place.parent != BlockParent::Detached),
        "nothing is ever created straight into Recently deleted"
    );
}

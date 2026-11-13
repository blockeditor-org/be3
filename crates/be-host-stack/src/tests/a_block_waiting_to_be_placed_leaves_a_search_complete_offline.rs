use uuid::Uuid;

use crate::search::Searching;

#[test]
fn a_block_waiting_to_be_placed_leaves_a_search_complete_offline() {
    let (block, missing) = (Uuid::new_v4(), Uuid::new_v4());
    let mut search = Searching::default();
    search.set_attached(true);
    search.wait_to_place(block, missing);
    assert!(!search.indexed(), "online, the block waits for the mirror");
    search.set_attached(false);
    assert!(
        search.indexed(),
        "offline, the mirror lists nothing, so the block does not hold the search incomplete"
    );
}

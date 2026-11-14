use super::*;

use be_block_content::Root;
use uuid::Uuid;

#[test]
fn a_connected_terminal_is_a_reference() {
    let first = Uuid::from_u128(1);
    let second = Uuid::from_u128(2);
    let boat = edited(
        &BoatContent::default(),
        [
            Boat::add_terminal(second, "bx_r4t8w3ne"),
            Boat::add_terminal(first, "bx_r4t8w3ne"),
            Boat::add_terminal(Uuid::from_u128(3), "bx_k2m9p4qa"),
            Boat::remove_terminal(Uuid::from_u128(3)),
        ],
    );
    let root = boat.root();
    assert_eq!(root.references(), vec![first, second]);
    assert_eq!(root.terminals_of("bx_r4t8w3ne"), vec![first, second]);
    assert!(root.terminals_of("bx_k2m9p4qa").is_empty());
}

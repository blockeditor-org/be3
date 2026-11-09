use super::*;

#[test]
fn an_empty_list_says_so() {
    let mut boat = Boat::listing(EMPTY);

    assert!(boat.test.shown("boat.empty"));
    boat.test.snapshot("an_empty_list_says_so");
}

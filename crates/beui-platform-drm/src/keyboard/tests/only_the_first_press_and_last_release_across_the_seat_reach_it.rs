use super::*;

#[test]
fn only_the_first_press_and_last_release_across_the_seat_reach_it() {
    assert!(
        seat_edge(true, 1),
        "the first keyboard to press a key presses it"
    );
    assert!(
        !seat_edge(true, 2),
        "a second keyboard pressing it changes nothing"
    );
    assert!(
        !seat_edge(false, 1),
        "letting go on one keyboard leaves it held on the other"
    );
    assert!(seat_edge(false, 0), "the last release lets it go");
}

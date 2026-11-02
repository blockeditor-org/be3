use super::*;

#[test]
fn the_first_move_is_one_of_seven() {
    let offered = labels(&[], FIRST);

    assert_eq!(
        offered,
        ["12-16", "11-16", "11-15", "10-15", "10-14", "9-14", "9-13"]
    );
    assert_eq!(show(&[], FIRST).description, "Your move (Dark)");
}

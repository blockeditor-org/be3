use super::*;

#[test]
fn a_child_moved_to_the_end_is_the_only_one_that_moves() {
    let kept = order::longest_increasing(&[Some(1), Some(2), Some(3), Some(0), None]);
    assert_eq!(kept, [true, true, true, false, false]);
}

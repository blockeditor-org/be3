use super::*;

#[test]
fn a_placer_is_never_held_behind_a_child_it_holds() {
    let held = held_behind(vec![
        (1, "files", "text"),
        (2, "text", "files"),
        (3, "text", "chat"),
    ]);
    assert_eq!(
        held,
        ["text", "chat"].into_iter().collect(),
        "the second edge would close a cycle with the first"
    );
}

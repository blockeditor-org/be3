use super::*;

#[test]
fn a_leave_asked_while_another_is_pending_does_not_change_what_confirm_leaves() {
    let (shown, swapped) = (Uuid::new_v4(), Uuid::new_v4());
    let mut pending = None;
    ask_to_leave(&mut pending, shown);
    ask_to_leave(&mut pending, swapped);
    assert_eq!(pending, Some(shown), "a second ask waits for the first");
    assert_eq!(confirmed_leave(pending, shown), Some(shown));
    assert_eq!(
        confirmed_leave(Some(swapped), shown),
        None,
        "a confirm never leaves a block the dialog did not show"
    );
    assert_eq!(confirmed_leave(None, shown), None);
}

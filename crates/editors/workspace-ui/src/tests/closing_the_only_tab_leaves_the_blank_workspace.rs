use super::*;

#[test]
fn closing_the_only_tab_leaves_the_blank_workspace() {
    let (mut fixture, opened) = editor();

    show(&mut fixture, opened, None);
    assert_eq!(fixture.shown(), vec![opened]);

    fixture.close_active_tab();

    assert!(
        fixture.shown().is_empty(),
        "closing the last tab leaves no block on show"
    );
    assert_eq!(fixture.focused(), None);
    assert!(
        fixture.says("No file open"),
        "closing the last tab leaves the blank workspace"
    );
}

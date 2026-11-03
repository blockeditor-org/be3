use super::*;

#[test]
fn windows_that_close_as_the_session_ends_are_kept() {
    let mut fixture = Fixture::new();
    fixture.settle();
    fixture.allow_power(EVERYTHING);
    fixture.show_windows(&[foot(1), notes(2)]);
    let tabs = vec![fixture.tab_of(1), fixture.tab_of(2)];

    fixture.allow_power(ending(SessionEnd::LogOut));
    fixture.show_windows(&[notes(2)]);
    assert_eq!(
        kept_keys(&fixture),
        tabs,
        "a program closing because the session ends is saved as it closes"
    );
    assert!(
        fixture.placeholder_rect(tabs[0]).is_some(),
        "and its window shows a placeholder at once"
    );
    fixture.show_windows(&[]);

    assert_eq!(
        kept_keys(&fixture),
        tabs,
        "the programs closing because the session ends are still there to bring back"
    );
    assert!(
        tabs.iter()
            .all(|tab| fixture.placeholder_rect(*tab).is_some()),
        "as placeholders"
    );
}

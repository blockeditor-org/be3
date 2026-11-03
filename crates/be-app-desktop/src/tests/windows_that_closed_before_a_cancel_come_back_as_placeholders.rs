use super::*;

#[test]
fn windows_that_closed_before_a_cancel_come_back_as_placeholders() {
    for end in [SessionEnd::LogOut, SessionEnd::Restart, SessionEnd::Close] {
        let mut fixture = Fixture::new();
        fixture.settle();
        fixture.allow_power(EVERYTHING);
        fixture.show_windows(&[foot(1), notes(2), host_window(3, true)]);
        let tabs = [1, 2, 3].map(|id| fixture.tab_of(id));

        fixture.allow_power(ending(end));
        fixture.show_windows(&[host_window(3, true)]);
        fixture.allow_power(EVERYTHING);
        fixture.settle();

        assert_eq!(
            kept_keys(&fixture),
            tabs.to_vec(),
            "cancelling, or a restart that failed, loses nothing: {end:?}"
        );
        assert!(
            fixture.placeholder_rect(tabs[0]).is_some() && fixture.placeholder_rect(tabs[1]).is_some(),
            "the windows that closed wait as placeholders to be opened again"
        );
        assert!(
            fixture.window_rect(3).is_some(),
            "and the one that would not close is still open"
        );

        fixture.show_windows(&[]);
        assert_eq!(
            kept_keys(&fixture),
            tabs[..2].to_vec(),
            "once the session goes on, closing a window is closing it"
        );
    }
}

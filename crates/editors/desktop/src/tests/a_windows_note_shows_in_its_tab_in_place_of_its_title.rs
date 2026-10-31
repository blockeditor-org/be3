use super::*;

#[test]
fn a_windows_note_shows_in_its_tab_in_place_of_its_title() {
    let mut fixture = Fixture::with_windows(&[1, 2]);
    assert!(fixture.says("notes-1.txt"));
    let noted = HostWindow {
        note: Some("Ctrl+Alt+Space comes back to Block".to_owned()),
        ..host_window(1, true)
    };
    fixture.show_windows(&[noted, host_window(2, false)]);
    assert!(
        fixture.says("Ctrl+Alt+Space comes back to Block"),
        "the note is in the window's tab"
    );
    assert_eq!(
        fixture.times_said("notes-1.txt"),
        1,
        "the note takes the title's place in the tab while it is set, and the bar's chip keeps it"
    );
    fixture.test.snapshot("a_windows_note_in_its_tab");

    fixture.show_windows(&[host_window(1, true), host_window(2, false)]);
    assert!(!fixture.says("Ctrl+Alt+Space comes back to Block"));
    assert!(fixture.says("notes-1.txt"), "the title comes back");
}

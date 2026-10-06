use super::*;

#[test]
fn a_desktop_session_reopens_its_tabs_in_its_window() {
    let (mut fixture, opened) = desktop(None);
    show(&mut fixture, opened, None);
    let layout = profile(&fixture)
        .state("layout")
        .cloned()
        .expect("the layout is saved in the profile");

    let (mut reopened, _) = desktop(Some(layout));
    reopened.settle();

    assert_eq!(reopened.open_tabs(), 1, "the tab comes back");
    assert!(reopened.shown().contains(&opened));
    assert!(reopened.says("Workspace"), "it is still in the group");
    let placed = reopened.test.children();
    assert!(
        placed
            .iter()
            .all(|placement| placement.rect.x > 0.0 && placement.rect.y > 0.0),
        "the files and the tab still float in the window"
    );

    reopened.close_active_tab();
    assert!(
        reopened.says("No file open"),
        "closing the tab leaves the empty pane"
    );
    assert!(reopened.says("Workspace"), "the group stays");
}

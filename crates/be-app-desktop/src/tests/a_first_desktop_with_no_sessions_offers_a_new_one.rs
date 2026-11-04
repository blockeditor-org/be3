use super::*;

#[test]
fn a_first_desktop_with_no_sessions_offers_a_new_one() {
    let (mut fixture, _) = Fixture::new().with_sessions(&[("Desktop", DESKTOP_EDITOR)]);
    fixture.settle();

    fixture.test.click("desktop.apps");
    fixture.settle();
    assert!(
        fixture.test.shown("desktop.apps.sessions.new"),
        "the Apps panel opens with a way to start a session"
    );
    assert!(
        !fixture.says("Desktop"),
        "the desktop session itself is not offered"
    );
    fixture.test.snapshot("the_apps_panel_on_a_first_desktop");

    fixture.test.click("desktop.apps.sessions.new");
    fixture.settle();
    assert!(
        !fixture.test.shown("desktop.apps.panel"),
        "starting a session closes the panel"
    );
    let placed = fixture.placed_blocks();
    let [(session, view, _)] = placed[..] else {
        panic!("the new session opens, and only it: {placed:?}");
    };
    assert_eq!(view, Some(session), "the session is its own view");
    let info = fixture
        .test
        .store()
        .block(session)
        .expect("the session exists");
    assert_eq!(info.parent, Some(BlockParent::Block(fixture.settings)));
    assert_eq!(info.name().as_deref(), Some("Session 1"));
    let settings: SettingsContent = fixture.test.content(Some(fixture.settings));
    assert!(
        settings.root().sessions().contains(&session),
        "the session is one of the account's sessions"
    );
    assert_eq!(info.metadata.get(&VIEW_EDITOR), Some(WORKSPACE_EDITOR));

    fixture.close_a_tab();
    fixture.test.click("desktop.apps");
    fixture.settle();
    assert!(fixture.says("Session 1"), "the panel lists the new session");
}

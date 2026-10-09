use super::*;

#[test]
fn a_first_desktop_with_no_sessions_offers_a_new_one() {
    let (mut fixture, _) = Fixture::new().with_sessions(&[("Desktop", LINUX_DESKTOP_EDITOR)]);
    fixture.settle();

    fixture.test.click("desktop.sessions");
    fixture.settle();
    assert!(
        fixture
            .test
            .document()
            .find_test_id("desktop.sessions.new")
            .is_some(),
        "the menu opens with a way to start a session"
    );
    assert!(
        !fixture.says("Desktop"),
        "the desktop itself is not offered"
    );
    fixture
        .test
        .snapshot("the_sessions_menu_on_a_first_desktop");

    fixture.test.click("desktop.sessions.new");
    fixture.settle();
    let placed = fixture.placed_blocks();
    let [(profile, view, _)] = placed[..] else {
        panic!("the new session opens, and only it: {placed:?}");
    };
    assert_eq!(view, Some(profile), "the session is its own view");
    let info = fixture
        .test
        .store()
        .block(profile)
        .expect("the session exists");
    assert_eq!(info.parent, BlockParent::Block(fixture.settings));
    assert_eq!(info.name.as_deref(), Some("Session 1"));
    let settings: SettingsContent = fixture.test.content(Some(fixture.settings));
    assert!(
        settings.root().profiles().contains(&profile),
        "the session is one of the account's profiles"
    );
    let shown: EditorViewContent = fixture.test.content(Some(profile));
    assert_eq!(shown.root().editor, WORKSPACE_EDITOR);

    fixture.close_a_tab();
    fixture.test.click("desktop.sessions");
    fixture.settle();
    assert!(fixture.says("Session 1"), "the menu lists the new session");
}

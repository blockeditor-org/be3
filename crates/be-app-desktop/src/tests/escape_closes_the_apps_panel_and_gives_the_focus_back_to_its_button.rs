use super::*;

#[test]
fn escape_closes_the_apps_panel_and_gives_the_focus_back_to_its_button() {
    let mut fixture = Fixture::new();
    fixture.settle();
    fixture.test.click("desktop.apps");
    fixture.settle();
    assert!(fixture.test.shown("desktop.apps.panel"));
    assert!(
        fixture.test.wants_keyboard(),
        "the panel's search takes the focus and asks for the keyboard"
    );

    fixture.test.key(Key::Escape, Modifiers::NONE);
    fixture.settle();
    assert!(!fixture.test.shown("desktop.apps.panel"));
    let button = fixture
        .test
        .document()
        .find_test_id("desktop.apps")
        .expect("the bar has an Apps button");
    assert!(
        fixture.test.document().focus_is_within(button),
        "closing it gives the focus back to the button that opened it"
    );
    assert!(!fixture.test.wants_keyboard());
}

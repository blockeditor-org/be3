use super::*;

#[test]
fn escape_closes_the_launcher_and_gives_the_focus_back_to_its_button() {
    let mut fixture = Fixture::new();
    fixture.settle();
    fixture.test.click("desktop.launcher");
    fixture.settle();
    assert!(fixture.test.shown("launcher"));
    let button = fixture
        .test
        .document()
        .find_test_id("desktop.launcher")
        .expect("the bar has a programs button");
    assert!(
        !fixture.test.document().focus_is_within(button),
        "the launcher takes the focus"
    );

    fixture.test.key_press(Key::Escape);
    fixture.settle();
    assert!(!fixture.test.shown("launcher"));
    assert!(
        fixture.test.document().focus_is_within(button),
        "closing it gives the focus back to the button that opened it"
    );
}

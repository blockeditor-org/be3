use super::*;

#[test]
fn an_app_profile_fills_the_screen_without_a_desktop_bar() {
    let (fixture, _) = profiled(None);

    assert!(fixture.says("No file open"));
    assert!(
        fixture
            .test
            .document()
            .find_test_id("workspace.desktop-bar")
            .is_none(),
        "the bar belongs to a desktop session only"
    );
    assert!(
        !fixture.says("Workspace"),
        "the split view is not wrapped in a group"
    );
}

use super::*;

#[test]
fn a_modal_window_sends_presses_on_its_parent_to_itself() {
    let (mut fixture, _) = editor();
    let modal = TestWindow {
        modal: true,
        ..window(2, "Open file", Some(1))
    };
    fixture.show_windows(&[window(1, "Text Editor", None), modal]);
    fixture.settle();
    let parent = placed_windows(&fixture)
        .into_iter()
        .find(|(id, _)| *id == 1)
        .map(|(_, rect)| rect)
        .expect("the parent is placed");
    fixture.focused_windows();

    fixture
        .test
        .click(parent.max - be_sdk_beui::vec2(12.0, 12.0));
    fixture.settle();

    let asked = fixture.focused_windows();
    assert!(
        asked.contains(&2),
        "a press on the parent gives the dialog the keyboard: {asked:?}"
    );
    assert!(
        !asked.contains(&1),
        "the parent is not given the keyboard: {asked:?}"
    );
}

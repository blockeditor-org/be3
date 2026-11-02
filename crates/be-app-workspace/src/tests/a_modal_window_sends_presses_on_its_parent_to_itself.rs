use super::*;

#[test]
fn a_modal_window_sends_presses_on_its_parent_to_itself() {
    let (mut fixture, _) = editor();
    let modal = be_sdk_beui::HostWindow {
        modal: true,
        ..window(2, "Open file", Some(1))
    };
    fixture
        .host
        .set_watched::<WindowsWatch>(&vec![window(1, "Text Editor", None), modal]);
    fixture.settle();
    let parent = placed_windows(&fixture)
        .into_iter()
        .find(|(id, _)| *id == 1)
        .map(|(_, rect)| rect)
        .expect("the parent is placed");
    fixture.test.take_host_calls(be_sdk_test::requested::window);

    fixture
        .test
        .click(parent.max - be_sdk_beui::vec2(12.0, 12.0));
    fixture.settle();

    let asked = fixture.test.take_host_calls(be_sdk_test::requested::window);
    assert!(
        asked.contains(&WindowAction::Focus(be_sdk_beui::HostWindowId(2))),
        "a press on the parent gives the dialog the keyboard: {asked:?}"
    );
    assert!(
        !asked.contains(&WindowAction::Focus(be_sdk_beui::HostWindowId(1))),
        "the parent is not given the keyboard: {asked:?}"
    );
}

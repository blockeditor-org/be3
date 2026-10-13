use block_editor_beui::be_block::ViewState;

use super::*;

#[test]
fn a_calendar_the_desktop_no_longer_holds_is_replaced() {
    let mut fixture = Fixture::new();
    let gone = Uuid::new_v4();
    let mut desktop = EditorView::document(LINUX_DESKTOP_EDITOR, None);
    let remember = desktop.root().set_state(
        "desktop.calendar",
        Some(&ViewState::new(&(), vec![gone])),
        1,
        fixture.client,
    );
    desktop.apply(&remember);
    fixture.test.hold(None, desktop);
    fixture.settle();

    fixture.test.click("desktop.clock");
    fixture.settle();
    let calendar = fixture
        .calendar()
        .expect("the clock opens a calendar block");
    assert_ne!(
        calendar, gone,
        "a calendar that is no longer the desktop's child is not shown"
    );
    assert_eq!(fixture.test.store().created(), vec![calendar]);
}

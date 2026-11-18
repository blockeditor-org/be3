use be_sdk_beui::be_block_content::ViewRef;

use crate::root::calendar::KeptCalendar;

use super::*;

#[test]
fn a_calendar_the_desktop_no_longer_holds_is_replaced() {
    let mut fixture = Fixture::new();
    let gone = Uuid::new_v4();
    let mut desktop = EditorView::document(None);
    let remember = desktop
        .root()
        .set_view_state(Some(&KeptCalendar(ViewRef::new(gone))), 1, fixture.client)
        .expect("encodes")
        .expect("a change");
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
    let created = fixture.test.store().created();
    assert_eq!(
        (created.len(), created.last()),
        (3, Some(&calendar)),
        "a schema, a database and its calendar view are created"
    );
}

use super::*;
use crate::reactive::{NodeRef, with_reactive_scope};
use crate::styled::ModalSheet;

#[test]
fn a_fitted_sheet_taller_than_the_screen_stops_short_of_the_top_and_scrolls() {
    let rows = NodeRef::new();
    let (open, set_open) = create_signal(false);
    let document = build({
        let rows = rows.clone();
        move || {
            view! {
                <List spacing=0.0>
                    <Frame @sizing=ItemSize::Percent(100.0) />
                    <ModalSheet open={open} fit=true on_close={|| ()}>
                        <List @node_ref=&rows spacing=0.0>
                            <ForEach keys={indices(SHEET_ROWS)}>
                                {|_: usize| view! {
                                    <Frame height=SHEET_ROW_HEIGHT />
                                }}
                            </ForEach>
                        </List>
                    </ModalSheet>
                </List>
            }
        }
    });
    let mut harness = Harness::sized(document, Vec2::new(400.0, 600.0));
    harness.frame(Vec::new());
    with_reactive_scope(harness.document_mut(), move || set_open.set(true));
    harness.frame(Vec::new());
    let top = harness.rect(rows.get()).top();
    assert!(
        (top - 85.0).abs() < 1.0,
        "a fitted sheet holding more than fits stops at its top stop, \
         leaving its content starting at {top}"
    );

    harness.finger_drag_and_hold(pos2(200.0, 500.0), pos2(200.0, 300.0));
    harness.settle();
    let scrolled = harness.rect(rows.get()).top();
    assert!(
        scrolled < top - 150.0,
        "a swipe up scrolls what it holds, to {scrolled}"
    );
}

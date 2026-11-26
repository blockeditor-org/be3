use super::*;
use crate::primitives::Frame;
use crate::primitives::List;
use crate::reactive::*;
use crate::styled::*;

#[test]
fn a_fitted_sheet_taller_than_half_the_screen_stops_at_the_middle_and_scrolls() {
    let _root = crate::reactive::RootScope::new();
    let rows = NodeRef::new();
    let open = create_signal(false);
    let document = build_node(move || {
        view! {
            <List spacing=0.0>
                <ListItem grow=1.0>
                    <Frame />
                </ListItem>
                <ModalSheet open={open} fit=true on_close={|| ()}>
                    <List @node_ref=&rows spacing=0.0>
                        <ForEach value={indices(SHEET_ROWS)}>|_: usize|
                            <Frame height=SHEET_ROW_HEIGHT />
                        </ForEach>
                    </List>
                </ModalSheet>
            </List>
        }
    });
    let mut harness = Harness::sized(document, Vec2::new(400.0, 600.0));
    harness.frame(Vec::new());
    open.set(true);
    harness.settle();
    let top = harness.rect(rows.get()).top();
    let handle = harness.rect(harness.find("sheet.handle")).top();
    assert_eq!(
        handle, 300.0,
        "a fitted sheet holding more than half the screen rises only to the middle stop"
    );
    assert!(
        (top - 325.0).abs() < 1.0,
        "leaving its content starting under its grip, at {top}"
    );

    harness.finger_drag_and_hold(pos2(200.0, 500.0), pos2(200.0, 300.0));
    harness.settle();
    let scrolled = harness.rect(rows.get()).top();
    assert!(
        scrolled < top - 150.0,
        "a swipe up scrolls what it holds, to {scrolled}"
    );
    assert_eq!(
        harness.rect(harness.find("sheet.handle")).top(),
        300.0,
        "and the sheet stays at the middle stop"
    );
}

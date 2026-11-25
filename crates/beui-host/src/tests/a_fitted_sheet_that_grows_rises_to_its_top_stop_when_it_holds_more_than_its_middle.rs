use super::*;
use crate::primitives::Frame;
use crate::primitives::List;
use crate::reactive::*;
use crate::styled::*;

#[test]
fn a_fitted_sheet_that_grows_rises_to_its_top_stop_when_it_holds_more_than_its_middle() {
    let _root = crate::reactive::RootScope::new();
    let rows = create_signal(2_usize);
    let open = create_signal(false);
    let document = build_node(move || {
        view! {
            <List spacing=0.0>
                <ListItem grow=1.0>
                    <Frame />
                </ListItem>
                <ModalSheet open={open} fit=true grows=true on_close={|| ()}>
                    <List spacing=0.0>
                        <Dynamic value={rows}>|count: usize|
                            <ForEach value={indices(count)}>|_: usize|
                                <Frame height=SHEET_ROW_HEIGHT />
                            </ForEach>
                        </Dynamic>
                    </List>
                </ModalSheet>
            </List>
        }
    });
    let size = Vec2::new(400.0, 600.0);
    let mut harness = Harness::sized(document, size);
    harness.frame(Vec::new());
    open.set(true);
    harness.settle();
    let short = harness.rect(harness.find("sheet.handle")).top();
    assert!(
        short > size.y / 2.0,
        "holding little, it rests at its own height, below the middle: {short}"
    );

    open.set(false);
    harness.settle();
    rows.set(SHEET_ROWS);
    open.set(true);
    harness.settle();
    let tall = harness.rect(harness.find("sheet.handle")).top();
    let top = size.y * (1.0 - SHEET_STOPS[2]);
    assert!(
        (tall - top).abs() < 0.5,
        "holding more than the middle stop, it rises to the top stop: {tall}, not {top}"
    );
}

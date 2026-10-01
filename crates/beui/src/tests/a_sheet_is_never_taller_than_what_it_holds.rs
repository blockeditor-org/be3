use super::*;
use crate::reactive::NodeRef;

#[test]
fn a_sheet_is_never_taller_than_what_it_holds() {
    let sheet = NodeRef::new();
    let rows = NodeRef::new();
    let document = build({
        let (sheet, rows) = (sheet.clone(), rows.clone());
        move || {
            view! {
                <List spacing=0.0>
                    <Frame @sizing=ItemSize::Percent(100.0) />
                    <styled::Sheet @node_ref=&sheet extent=600.0 rest=0.9 on_close={|| ()}>
                        <List @node_ref=&rows spacing=0.0>
                            <ForEach keys={indices(3)}>
                                {|_: usize| view! {
                                    <Frame height=SHEET_ROW_HEIGHT />
                                }}
                            </ForEach>
                        </List>
                    </styled::Sheet>
                </List>
            }
        }
    });
    let mut harness = Harness::sized(document, Vec2::new(400.0, 600.0));
    harness.frame(Vec::new());
    let sheet = sheet.get();
    let rows = harness.rect(rows.get());
    assert_eq!(rows.height(), 3.0 * SHEET_ROW_HEIGHT);
    assert_eq!(
        harness.rect(sheet).bottom(),
        rows.bottom(),
        "a sheet resting at a stop taller than its content stops at the content's end"
    );
    assert_eq!(
        harness.rect(sheet).bottom(),
        600.0,
        "and sits on the bottom edge"
    );
}

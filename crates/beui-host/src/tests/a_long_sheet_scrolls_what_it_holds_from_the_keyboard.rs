use super::*;
use crate::primitives::Frame;
use crate::reactive::*;

/// What a sheet holds scrolls as any scroll does: from the keyboard too, once the sheet is up.
#[test]
fn a_long_sheet_scrolls_what_it_holds_from_the_keyboard() {
    let _root = RootScope::new();
    let sheet = NodeRef::new();
    let document = build_node(move || {
        view! {
            <ModalSheet @node_ref=&sheet open=true rest=0.9 on_close={|| ()}>
                <Frame height=2000.0 />
            </ModalSheet>
        }
    });
    let mut harness = Harness::sized(document, vec2(400.0, 600.0));
    harness.settle();
    let offset = |harness: &Harness| harness.document().scroll_offset(sheet.get());
    assert_eq!(offset(&harness), 540.0, "up at its top stop");

    harness.click(pos2(200.0, 300.0));
    harness.key(Key::PageDown, Modifiers::NONE);
    harness.settle();
    assert_eq!(
        offset(&harness),
        1080.0,
        "Page Down scrolls what it holds by a page"
    );
    harness.key(Key::Home, Modifiers::NONE);
    harness.settle();
    assert_eq!(
        offset(&harness),
        540.0,
        "and Home takes it back to its start, leaving the sheet up"
    );
}

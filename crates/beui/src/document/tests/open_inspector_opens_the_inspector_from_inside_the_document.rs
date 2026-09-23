use super::*;
use crate::reactive::{view, with_document};

#[test]
fn open_inspector_opens_the_inspector_from_inside_the_document() {
    let (document, [button]) = toolbar_of(|| {
        [view! {
            <LabelledButton label="Inspect" on_click={|| with_document(Document::open_inspector)} />
        }]
    });
    let mut harness = Harness::sized(document, WIDE_VIEWPORT);
    harness.frame(Vec::new());

    harness.click(harness.center(button));
    harness.frame(Vec::new());

    assert!(harness.document.inspector.is_some());
}

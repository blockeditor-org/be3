use super::*;
use crate::native::Document;
use crate::native::with_document;
use crate::reactive::*;

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

    assert!(harness.inspector_open());
}

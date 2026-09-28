use super::*;
use crate::reactive::{Frame, List, Show, build, create_signal, view, with_reactive_scope};

#[test]
fn a_test_id_names_the_copy_that_is_shown() {
    let (wide, set_wide) = create_signal(true);
    let widen = set_wide.clone();
    let document = build(move || {
        let shown = wide.clone();
        let narrow = crate::reactive::create_memo(move || !shown.get());
        view! {
            <List spacing=0.0>
                <Show condition={wide}>
                    <Frame @test_id="tool" width=10.0 height=10.0 />
                </Show>
                <Show condition={narrow}>
                    <Frame @test_id="tool" width=20.0 height=20.0 />
                </Show>
            </List>
        }
    });
    let mut harness = Harness::new(document);
    harness.frame(Vec::new());
    let width = |harness: &Harness| {
        let id = harness
            .document()
            .find_test_id("tool")
            .expect("a node named tool");
        harness.document().node_rect(id).map(|rect| rect.height())
    };
    assert_eq!(width(&harness), Some(10.0));

    with_reactive_scope(harness.document_mut(), move || set_wide.set(false));
    harness.frame(Vec::new());
    assert_eq!(
        width(&harness),
        Some(20.0),
        "the id names the copy on screen"
    );

    with_reactive_scope(harness.document_mut(), move || widen.set(true));
    harness.frame(Vec::new());
    assert_eq!(
        width(&harness),
        Some(10.0),
        "a copy shown again is named, not the hidden one built after it"
    );
}

use super::*;
use crate::primitives::Frame;
use crate::primitives::List;
use crate::reactive::*;
use crate::styled::*;

#[test]
fn a_modal_sheet_fits_its_content_and_a_tap_above_it_closes_it() {
    let _root = crate::reactive::RootScope::new();
    let content = NodeRef::new();
    let closed = create_signal(0);
    let open = create_signal(false);
    let document = build_node(move || {
        view! {
            <List spacing=0.0>
                <Frame @sizing=ItemSize::Percent(100.0) />
                <ModalSheet
                    open={open}
                    fit=true
                    on_close={move || closed.update(|closed| *closed += 1)}
                >
                    <Frame @node_ref=&content height=120.0 />
                </ModalSheet>
            </List>
        }
    });
    let mut harness = Harness::sized(document, Vec2::new(400.0, 600.0));
    harness.frame(Vec::new());
    assert!(
        content.try_get().is_none(),
        "a closed modal sheet builds nothing it holds"
    );

    open.set(true);
    harness.settle();
    let body = harness.rect(content.get());
    assert_eq!(
        body.height(),
        120.0,
        "a fitted sheet is as tall as what it holds"
    );
    assert_eq!(body.bottom(), 600.0, "and it sits on the bottom edge");

    let outside = harness.find("sheet.outside");
    let above = harness.center(outside);
    harness.click(above);
    assert_eq!(closed.get_untracked(), 1, "a tap above the sheet closes it");

    harness.key(Key::Escape, Modifiers::NONE);
    assert_eq!(closed.get_untracked(), 2, "Escape closes a modal sheet");
}

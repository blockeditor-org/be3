use super::*;
use crate::reactive::{Frame, List, NodeRef, build, create_signal, view, with_reactive_scope};
use crate::styled::ModalSheet;

#[test]
fn a_modal_sheet_fits_its_content_and_a_tap_above_it_closes_it() {
    let content = NodeRef::new();
    let closed = Rc::new(Cell::new(0));
    let (open, set_open) = create_signal(false);
    let document = build({
        let (content, closed, open) = (content.clone(), closed.clone(), open.clone());
        move || {
            view! {
                <List spacing=0.0>
                    <Frame @sizing=ItemSize::Percent(100.0) />
                    <ModalSheet
                        open={open}
                        fit=true
                        on_close={move || closed.set(closed.get() + 1)}
                    >
                        <Frame @node_ref=&content height=120.0 />
                    </ModalSheet>
                </List>
            }
        }
    });
    let mut harness = Harness::sized(document, Vec2::new(400.0, 600.0));
    harness.frame(Vec::new());
    assert!(
        harness.document().node_rect(content.get()).is_none(),
        "a closed modal sheet shows nothing"
    );

    with_reactive_scope(harness.document_mut(), move || set_open.set(true));
    harness.frame(Vec::new());
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
    assert_eq!(closed.get(), 1, "a tap above the sheet closes it");

    harness.key(Key::Escape, Modifiers::NONE);
    assert_eq!(closed.get(), 2, "Escape closes a modal sheet");
}

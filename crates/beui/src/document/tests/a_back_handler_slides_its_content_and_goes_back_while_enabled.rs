use super::*;
use crate::input::{BackEdge, BackGesture};
use crate::reactive::{
    BackHandler, NodeRef, Text, build, create_signal, view, with_reactive_scope,
};

#[test]
fn a_back_handler_slides_its_content_and_goes_back_while_enabled() {
    let (enabled, set_enabled) = create_signal(false);
    let backs = Rc::new(Cell::new(0));
    let reports = backs.clone();
    let page = NodeRef::new();
    let page_ref = page.clone();
    let document = build({
        let enabled = enabled.clone();
        move || {
            view! {
                <BackHandler enabled={enabled} on_back={move || reports.set(reports.get() + 1)}>
                    <Text string="Page" font_size=14.0 color=Color32::WHITE @node_ref={&page_ref} />
                </BackHandler>
            }
        }
    });

    let mut harness = Harness::new(document);
    assert!(!harness.frame(Vec::new()).handles_back);
    harness.frame(vec![Event::Back(BackGesture::Invoked)]);
    assert_eq!(backs.get(), 0);

    with_reactive_scope(harness.document_mut(), move || set_enabled.set(true));
    assert!(harness.frame(Vec::new()).handles_back);
    let resting = harness.rect(page.get());

    harness.frame(vec![
        Event::Back(BackGesture::Started {
            edge: BackEdge::Left,
        }),
        Event::Back(BackGesture::Progressed(1.0)),
    ]);
    assert!(harness.rect(page.get()).left() > resting.left() + 20.0);

    harness.frame(vec![Event::Back(BackGesture::Invoked)]);
    assert_eq!(backs.get(), 1);
    harness.frame(Vec::new());
    assert_eq!(harness.rect(page.get()), resting);

    harness.key(Key::BrowserBack, Modifiers::NONE);
    assert_eq!(backs.get(), 2);
}

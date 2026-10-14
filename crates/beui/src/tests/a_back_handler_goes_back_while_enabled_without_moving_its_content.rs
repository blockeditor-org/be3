use super::*;
use crate::input::{BackEdge, BackGesture};
use crate::reactive::{
    BackHandler, NodeRef, Text, build, create_signal, view, with_reactive_scope,
};

#[test]
fn a_back_handler_goes_back_while_enabled_without_moving_its_content() {
    let _root = crate::reactive::RootScope::new();
    let (enabled, set_enabled) = create_signal(false);
    let (backs, set_backs) = create_signal(0);
    let page = NodeRef::new();
    let document = build({
        move || {
            view! {
                <BackHandler enabled={enabled} on_back={move || set_backs.update(|backs| *backs += 1)}>
                    <Text string="Page" font_size=14.0 color=Color32::WHITE @node_ref={&page} />
                </BackHandler>
            }
        }
    });

    let mut harness = Harness::new(document);
    assert!(!harness.frame(Vec::new()).window.handles_back);
    harness.frame(vec![Event::Back(BackGesture::Invoked)]);
    assert_eq!(backs.get_untracked(), 0);

    with_reactive_scope(harness.document_mut(), move || set_enabled.set(true));
    assert!(harness.frame(Vec::new()).window.handles_back);
    let resting = harness.rect(page.get());

    harness.frame(vec![
        Event::Back(BackGesture::Started {
            edge: BackEdge::Left,
        }),
        Event::Back(BackGesture::Progressed(1.0)),
    ]);
    assert_eq!(harness.rect(page.get()), resting);
    assert_eq!(backs.get_untracked(), 0);

    harness.frame(vec![Event::Back(BackGesture::Invoked)]);
    assert_eq!(backs.get_untracked(), 1);

    harness.key(Key::BrowserBack, Modifiers::NONE);
    assert_eq!(backs.get_untracked(), 2);
}

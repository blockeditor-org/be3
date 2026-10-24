use super::*;
use crate::primitives::*;
use crate::reactive::*;

#[test]
fn hover_only_repaints_when_its_handler_changes_a_node() {
    let _root = crate::reactive::RootScope::new();
    let hover_paints = create_signal(false);
    let fill = NodeRef::new();
    let mut document = build_node(move || {
        view! {
            <Interactive cursor=crate::CursorIcon::PointingHand on_hover_change={hover_paints}>
                <Frame @node_ref=&fill fill=Color32::WHITE radius=0.0 />
            </Interactive>
        }
    });
    let fill = kind_of::<FrameNode>(&document, fill.get());
    let (layouts, paints) = counted(&mut document, fill);
    let mut harness = Harness::new(document);
    harness.frame(vec![]);
    let output = harness.frame(vec![Input::PointerMoved(pos2(10.0, 10.0))]);
    assert!(!output.main().changed);
    assert_eq!(
        output.main().window.requests.cursor,
        crate::CursorIcon::PointingHand
    );
    assert_eq!((layouts.get_untracked(), paints.get_untracked()), (1, 1));
    with_installed(harness.document_mut(), |_| {
        create_effect(move || {
            let color = if hover_paints.get() {
                Color32::BLACK
            } else {
                Color32::WHITE
            };
            with_document(|document| document.set_frame_color(fill, color));
        });
    });
    harness.frame(vec![Input::PointerLeft]);
    assert!(
        harness
            .frame(vec![Input::PointerMoved(pos2(10.0, 10.0))])
            .main()
            .changed
    );
    let before = (layouts.get_untracked(), paints.get_untracked());
    assert!(
        !harness
            .frame(vec![Input::PointerMoved(pos2(20.0, 20.0))])
            .main()
            .changed
    );
    assert_eq!((layouts.get_untracked(), paints.get_untracked()), before);
    assert!(harness.frame(vec![Input::PointerLeft]).main().changed);
}

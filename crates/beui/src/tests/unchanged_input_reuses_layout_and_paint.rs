use super::*;
use crate::primitives::*;
use crate::reactive::*;

#[test]
fn unchanged_input_reuses_layout_and_paint() {
    let _root = crate::reactive::RootScope::new();
    let fill = NodeRef::new();
    let mut document = build_node(move || {
        view! {
            <Frame @node_ref=&fill fill=Color32::WHITE radius=0.0 />
        }
    });
    let fill = kind_of::<FrameNode>(&document, fill.get());
    let (layouts, paints) = counted(&mut document, fill);
    let mut harness = Harness::new(document);
    assert!(harness.frame(vec![]).main().changed);
    assert_eq!((layouts.get_untracked(), paints.get_untracked()), (1, 1));
    for event in [
        Input::PointerMoved(pos2(10.0, 10.0)),
        Input::PointerMoved(pos2(20.0, 20.0)),
        Input::PointerLeft,
        Input::Text("ignored".into()),
        key_event(Key::A, false),
    ] {
        let output = harness.frame(vec![event]);
        assert!(!output.main().changed);
        assert_eq!(output.main().shapes().len(), 1);
    }
    harness.document_mut().set_frame_color(fill, Color32::WHITE);
    harness.document_mut().set_root(fill.id());
    assert!(!harness.frame(vec![]).main().changed);
    assert_eq!((layouts.get_untracked(), paints.get_untracked()), (1, 1));
    harness.document_mut().set_frame_color(fill, Color32::BLACK);
    assert!(harness.frame(vec![]).main().changed);
    assert_eq!((layouts.get_untracked(), paints.get_untracked()), (1, 2));
}

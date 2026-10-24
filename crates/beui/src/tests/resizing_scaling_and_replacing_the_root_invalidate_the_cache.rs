use super::*;
use crate::primitives::*;
use crate::reactive::*;

#[test]
fn resizing_scaling_and_replacing_the_root_invalidate_the_cache() {
    let _root = crate::reactive::RootScope::new();
    let text = NodeRef::new();
    let mut document = build_node(move || {
        view! {
            <Text @node_ref=&text color=Color32::WHITE>"hello"</Text>
        }
    });
    let text = text.get();
    let (layouts, paints) = counted(&mut document, text);
    let mut harness = Harness::new(document);
    harness.frame(vec![]);
    harness.resize(WIDE_VIEWPORT);
    harness.frame(vec![]);
    assert_eq!(harness.rect(text).size(), WIDE_VIEWPORT);
    harness.rescale(2.0);
    assert!(harness.frame(vec![]).main().changed);
    assert_eq!((layouts.get_untracked(), paints.get_untracked()), (3, 3));
    harness.document_mut().remove_node(text);
    let output = harness.frame(vec![]);
    assert!(output.main().changed);
    assert!(output.main().shapes().is_empty());
    assert!(harness.document().node_rect(text).is_none());
    let fill = with_installed(harness.document_mut(), |_| {
        view! {
            <Frame fill=Color32::BLACK radius=0.0 />
        }
    });
    harness.document_mut().set_root(fill);
    assert!(harness.frame(vec![]).main().changed);
}

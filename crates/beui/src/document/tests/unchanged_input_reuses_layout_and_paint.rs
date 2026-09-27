use super::*;
use crate::painter::Shape;
use crate::reactive::{Frame, NodeRef, build, view};

#[test]
fn unchanged_input_reuses_layout_and_paint() {
    let fill = NodeRef::new();
    let mut document = build({
        let fill = fill.clone();
        move || {
            view! {
                <Frame @node_ref=&fill color=Color32::WHITE radius=0 />
            }
        }
    });
    let fill = fill.get();
    let (layouts, paints) = counted(&mut document, fill);
    let mut harness = Harness::new(document);
    let first = harness.frame(vec![]);
    assert!(first.changed);
    assert_eq!((layouts.get(), paints.get()), (1, 1));
    for event in [
        Event::PointerMoved(pos2(10.0, 10.0)),
        Event::PointerMoved(pos2(20.0, 20.0)),
        Event::PointerGone,
        Event::Text("ignored".into()),
        key_event(Key::A, false, Modifiers::NONE),
    ] {
        let output = harness.frame(vec![event]);
        assert!(!output.changed);
        assert!(Rc::ptr_eq(&first.shapes, &output.shapes));
        assert!(Rc::ptr_eq(&output.shapes, &harness.document.shapes));
        assert_eq!(output.shapes().len(), 1);
    }
    harness.document.set_frame_color(fill, Color32::WHITE);
    harness.document.set_root(fill);
    assert!(!harness.frame(vec![]).changed);
    assert_eq!((layouts.get(), paints.get()), (1, 1));
    harness.document.set_frame_color(fill, Color32::BLACK);
    let changed = harness.frame(vec![]);
    assert!(changed.changed);
    assert!(!Rc::ptr_eq(&first.shapes, &changed.shapes));
    assert!(matches!(&first.shapes()[0], Shape::Rect { color, .. } if *color == Color32::WHITE));
    assert_eq!((layouts.get(), paints.get()), (1, 2));
}

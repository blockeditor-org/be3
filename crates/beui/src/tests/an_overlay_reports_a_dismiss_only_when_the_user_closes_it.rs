use super::*;
use crate::base::overlay::{OverlayAnchor, OverlayNode, Placement};
use crate::input::BackGesture;
use crate::reactive::{
    Frame, NodeRef, Overlay, WriteSignal, build, create_signal, view, with_document,
    with_reactive_scope,
};

#[test]
fn an_overlay_reports_a_dismiss_only_when_the_user_closes_it() {
    let (open, set_open) = create_signal(false);
    let (nested_open, set_nested_open) = create_signal(false);
    let dismissed = Rc::new(Cell::new(0));
    let nested_dismissed = Rc::new(Cell::new(0));
    let (outer, inner) = (NodeRef::new(), NodeRef::new());
    let document = build({
        let (open, nested_open) = (open.clone(), nested_open.clone());
        let (reports, nested_reports) = (dismissed.clone(), nested_dismissed.clone());
        let (outer, inner) = (outer.clone(), inner.clone());
        move || {
            view! {
                <Frame>
                    <Overlay
                        @node_ref=&outer
                        anchor=OverlayAnchor::Point(pos2(10.0, 10.0))
                        placement=Placement::At
                        open={open}
                        on_dismiss={move || reports.set(reports.get() + 1)}
                    >
                        <List spacing=0.0>
                            <Frame width=40.0 height=30.0 />
                            <Overlay
                                @node_ref=&inner
                                anchor=OverlayAnchor::Point(pos2(60.0, 10.0))
                                placement=Placement::At
                                open={nested_open}
                                on_dismiss={move || nested_reports.set(nested_reports.get() + 1)}
                            >
                                <Frame width=40.0 height=30.0 />
                            </Overlay>
                        </List>
                    </Overlay>
                </Frame>
            }
        }
    });
    let mut harness = Harness::new(document);
    harness.frame(Vec::new());
    let outer = kind_of::<OverlayNode>(harness.document(), outer.get());
    let inner = kind_of::<OverlayNode>(harness.document(), inner.get());

    let set = |harness: &mut Harness, signal: &WriteSignal<bool>, value: bool| {
        let signal = signal.clone();
        with_reactive_scope(harness.document_mut(), move || signal.set(value));
        harness.frame(Vec::new());
    };
    let reopen = |harness: &mut Harness| {
        set(harness, &set_open, false);
        set(harness, &set_open, true);
        assert!(
            harness.document().is_overlay_open(outer.id()),
            "the overlay opens again"
        );
    };

    set(&mut harness, &set_open, true);
    assert!(harness.document().is_overlay_open(outer.id()));

    harness.key(Key::Escape, Modifiers::NONE);
    assert!(!harness.document().is_overlay_open(outer.id()));
    assert_eq!(dismissed.get(), 1, "Escape dismisses it");
    reopen(&mut harness);

    harness.click(pos2(VIEWPORT.x - 5.0, VIEWPORT.y - 5.0));
    harness.frame(Vec::new());
    assert!(!harness.document().is_overlay_open(outer.id()));
    assert_eq!(dismissed.get(), 2, "a click outside dismisses it");
    reopen(&mut harness);

    harness.frame(vec![Event::Back(BackGesture::Invoked)]);
    assert!(!harness.document().is_overlay_open(outer.id()));
    assert_eq!(dismissed.get(), 3, "back dismisses it");
    reopen(&mut harness);

    set(&mut harness, &set_open, false);
    assert!(!harness.document().is_overlay_open(outer.id()));
    assert_eq!(dismissed.get(), 3, "its owner closing it is no dismiss");
    set(&mut harness, &set_open, true);
    assert!(harness.document().is_overlay_open(outer.id()));

    set(&mut harness, &set_nested_open, true);
    assert!(harness.document().is_overlay_open(inner.id()));
    set(&mut harness, &set_open, false);
    assert!(!harness.document().is_overlay_open(inner.id()));
    assert_eq!(dismissed.get(), 3);
    assert_eq!(
        nested_dismissed.get(),
        1,
        "a nested overlay closed by its parent closing is dismissed"
    );

    set(&mut harness, &set_nested_open, false);
    set(&mut harness, &set_open, true);
    set(&mut harness, &set_nested_open, true);
    assert!(harness.document().is_overlay_open(inner.id()));
    with_reactive_scope(harness.document_mut(), || {
        with_document(|document| document.dismiss_overlay(outer));
    });
    harness.frame(Vec::new());
    assert!(!harness.document().is_overlay_open(inner.id()));
    assert_eq!(dismissed.get(), 4, "the parent is dismissed once");
    assert_eq!(
        nested_dismissed.get(),
        2,
        "and takes the nested overlay with it"
    );

    reopen(&mut harness);
    set(&mut harness, &set_nested_open, false);
    set(&mut harness, &set_nested_open, true);
    assert!(harness.document().is_overlay_open(inner.id()));
    harness.key(Key::Escape, Modifiers::NONE);
    assert!(!harness.document().is_overlay_open(inner.id()));
    assert!(harness.document().is_overlay_open(outer.id()));
    assert_eq!(
        nested_dismissed.get(),
        3,
        "Escape dismisses the topmost only"
    );
    assert_eq!(dismissed.get(), 4);
}

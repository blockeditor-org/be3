use super::*;
use crate::Dismiss;
use crate::base::overlay::{OverlayAnchor, OverlayNode, Placement};
use crate::input::BackGesture;
use crate::native::{with_document, with_reactive_scope};
use crate::primitives::*;
use crate::reactive::*;

#[test]
fn an_overlay_reports_a_dismiss_only_when_the_user_closes_it() {
    let _root = crate::reactive::RootScope::new();
    let open = create_signal(false);
    let nested_open = create_signal(false);
    let dismissed = create_signal(Vec::new());
    let nested_dismissed = create_signal(Vec::new());
    let (outer, inner) = (NodeRef::new(), NodeRef::new());
    let document = build_node(move || {
        view! {
            <Frame>
                <Overlay
                    @node_ref=&outer
                    anchor=OverlayAnchor::Point(pos2(10.0, 10.0))
                    placement=Placement::At
                    open={open}
                    on_dismiss={move |reason: Dismiss| {
                        dismissed.update(|dismissed| dismissed.push(reason));
                        open.set(false);
                    }}
                >
                    <List spacing=0.0>
                        <Frame width=40.0 height=30.0 />
                        <Overlay
                            @node_ref=&inner
                            anchor=OverlayAnchor::Point(pos2(60.0, 10.0))
                            placement=Placement::At
                            open={nested_open}
                            on_dismiss={move |reason: Dismiss| {
                                nested_dismissed.update(|nested_dismissed| nested_dismissed.push(reason));
                                nested_open.set(false);
                            }}
                        >
                            <Frame width=40.0 height=30.0 />
                        </Overlay>
                    </List>
                </Overlay>
            </Frame>
        }
    });
    let mut harness = Harness::new(document);
    harness.frame(Vec::new());
    let outer = kind_of::<OverlayNode>(harness.document(), outer.get());

    let inner_open = |harness: &Harness| {
        inner
            .try_get()
            .is_some_and(|inner| harness.document().is_overlay_open(inner))
    };
    let set = |harness: &mut Harness, signal: Signal<bool>, value: bool| {
        signal.set(value);
        harness.frame(Vec::new());
    };
    let open_outer = |harness: &mut Harness| {
        set(harness, open, true);
        assert!(
            harness.document().is_overlay_open(outer.id()),
            "the overlay opens again"
        );
    };

    open_outer(&mut harness);
    harness.key(Key::Escape, Modifiers::NONE);
    assert!(!harness.document().is_overlay_open(outer.id()));
    assert_eq!(
        dismissed.get_untracked(),
        [Dismiss::Escape],
        "Escape asks it to close"
    );

    open_outer(&mut harness);
    harness.click(pos2(VIEWPORT.x - 5.0, VIEWPORT.y - 5.0));
    harness.frame(Vec::new());
    assert!(!harness.document().is_overlay_open(outer.id()));
    assert_eq!(
        dismissed.get_untracked().last(),
        Some(&Dismiss::OutsidePress),
        "a click outside asks it to close"
    );

    open_outer(&mut harness);
    harness.frame(vec![Input::Back(BackGesture::Invoked)]);
    assert!(!harness.document().is_overlay_open(outer.id()));
    assert_eq!(
        dismissed.get_untracked().last(),
        Some(&Dismiss::Back),
        "back asks it to close"
    );

    open_outer(&mut harness);
    set(&mut harness, open, false);
    assert!(!harness.document().is_overlay_open(outer.id()));
    assert_eq!(
        dismissed.with_untracked(|dismissed| dismissed.len()),
        3,
        "its owner closing it is no dismiss"
    );

    open_outer(&mut harness);
    set(&mut harness, nested_open, true);
    assert!(inner_open(&harness));
    set(&mut harness, open, false);
    assert!(!inner_open(&harness));
    assert_eq!(dismissed.with_untracked(|dismissed| dismissed.len()), 3);
    assert_eq!(
        nested_dismissed.get_untracked(),
        [Dismiss::WithParent],
        "a nested overlay closed by its parent closing is told so"
    );

    open_outer(&mut harness);
    set(&mut harness, nested_open, true);
    with_reactive_scope(harness.document_mut(), || {
        with_document(|document| document.request_dismiss(outer, Dismiss::Back));
    });
    harness.frame(Vec::new());
    assert!(!inner_open(&harness));
    assert_eq!(
        dismissed.with_untracked(|dismissed| dismissed.len()),
        4,
        "the parent is dismissed once"
    );
    assert_eq!(
        nested_dismissed.with_untracked(|nested_dismissed| nested_dismissed.len()),
        2,
        "and takes the nested overlay with it"
    );

    open_outer(&mut harness);
    set(&mut harness, nested_open, true);
    harness.key(Key::Escape, Modifiers::NONE);
    assert!(!inner_open(&harness));
    assert!(harness.document().is_overlay_open(outer.id()));
    assert_eq!(
        nested_dismissed.get_untracked().last(),
        Some(&Dismiss::Escape),
        "Escape asks the topmost only"
    );
    assert_eq!(dismissed.with_untracked(|dismissed| dismissed.len()), 4);
}

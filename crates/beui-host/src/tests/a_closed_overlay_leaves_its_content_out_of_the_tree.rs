use super::*;
use crate::base::overlay::OverlayNode;
use crate::primitives::*;
use crate::reactive::*;
use beui::layer::{OverlayAnchor, Placement};

#[test]
fn a_closed_overlay_leaves_its_content_out_of_the_tree() {
    let _root = crate::reactive::RootScope::new();
    let open = create_signal(false);
    let builds = create_signal(0);
    let (fresh, kept) = (NodeRef::new(), NodeRef::new());
    let (fresh_content, kept_content) = (NodeRef::new(), NodeRef::new());
    let document = build_node(move || {
        view! {
            <List spacing=0.0>
                <Overlay
                    @node_ref=&fresh
                    anchor=OverlayAnchor::Point(pos2(10.0, 10.0))
                    placement=Placement::At
                    open={open}
                    content={move || {
                        builds.update(|builds| *builds += 1);
                        view! {
                            <Frame @node_ref=&fresh_content width=40.0 height=30.0 />
                        }
                    }}
                />
                <Overlay
                    @node_ref=&kept
                    anchor=OverlayAnchor::Point(pos2(60.0, 10.0))
                    placement=Placement::At
                    open={open}
                    keep_alive=true
                >
                    <Frame @node_ref=&kept_content width=40.0 height=30.0 />
                </Overlay>
            </List>
        }
    });
    let mut harness = Harness::new(document);
    harness.frame(Vec::new());
    let (fresh, kept) = (fresh.get(), kept.get());
    let content = |harness: &Harness, overlay: NodeId| {
        harness
            .document()
            .overlay_content(kind_of::<OverlayNode>(harness.document(), overlay))
    };
    assert_eq!(builds.get_untracked(), 0, "a closed overlay builds nothing");
    assert!(fresh_content.try_get().is_none() && kept_content.try_get().is_none());

    open.set(true);
    harness.frame(Vec::new());
    assert_eq!(builds.get_untracked(), 1, "opening builds its content");
    assert_eq!(content(&harness, fresh), fresh_content.try_get());
    let first = kept_content.get();
    assert_eq!(content(&harness, kept), Some(first));

    open.set(false);
    harness.frame(Vec::new());
    assert_eq!(content(&harness, fresh), None, "closing takes it out");
    assert!(
        fresh_content.try_get().is_none(),
        "and an overlay that does not keep its content disposes of it"
    );
    assert_eq!(content(&harness, kept), None);
    assert!(
        harness.document().contains(first),
        "one that keeps it alive holds on to it out of the tree"
    );

    open.set(true);
    harness.frame(Vec::new());
    assert_eq!(builds.get_untracked(), 2, "opening again builds afresh");
    assert_eq!(
        content(&harness, kept),
        Some(first),
        "while the kept one shows what it built the first time"
    );
}

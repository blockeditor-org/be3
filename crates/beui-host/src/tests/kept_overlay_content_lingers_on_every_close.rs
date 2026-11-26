use super::*;
use crate::base::overlay::OverlayNode;
use crate::primitives::*;
use crate::reactive::*;
use beui::layer::{OverlayAnchor, Placement};
use beui_view::components::overlay::use_overlay_presence;

#[test]
fn kept_overlay_content_lingers_on_every_close() {
    let _root = crate::reactive::RootScope::new();
    let open = create_signal(false);
    let leaving = create_signal(false);
    let builds = create_signal(0);
    let overlay = NodeRef::new();
    let document = build_node(move || {
        view! {
            <KeepAlive>
                <Overlay
                    @node_ref=&overlay
                    anchor=OverlayAnchor::Point(pos2(10.0, 10.0))
                    placement=Placement::At
                    open={open}
                >||
                    <Kept>|| {
                        builds.update(|builds| *builds += 1);
                        // Read inside the kept subtree, with nothing carrying it in.
                        let presence = use_overlay_presence().expect("the kept content reads its overlay's presence");
                        presence.linger_while(create_memo(move || leaving.get()));
                        view! {
                            <Frame width=40.0 height=30.0 />
                        }
                    }</Kept>
                </Overlay>
            </KeepAlive>
        }
    });
    let mut harness = Harness::new(document);
    harness.frame(Vec::new());
    let overlay = overlay.get();
    let showing = |harness: &Harness| {
        harness
            .document()
            .overlay_content(kind_of::<OverlayNode>(harness.document(), overlay))
            .is_some()
    };
    for close in ["first", "second"] {
        open.set(true);
        leaving.set(true);
        harness.frame(Vec::new());
        open.set(false);
        harness.frame(Vec::new());
        assert!(
            showing(&harness),
            "the {close} close lingers while its content asks"
        );
        leaving.set(false);
        harness.frame(Vec::new());
        assert!(!showing(&harness), "and goes once it stops asking");
    }
    assert_eq!(
        builds.get_untracked(),
        1,
        "the content is built once and kept"
    );
}

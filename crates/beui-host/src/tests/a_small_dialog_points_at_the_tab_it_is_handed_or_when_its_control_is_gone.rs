use super::*;
use crate::geometry::Rect;
use crate::primitives::{Frame, List};
use crate::reactive::*;
use crate::styled::*;

const TAB: Rect = Rect {
    min: Pos2::new(120.0, 0.0),
    max: Pos2::new(240.0, 28.0),
};

#[test]
fn a_small_dialog_points_at_the_tab_it_is_handed_or_when_its_control_is_gone() {
    let _root = crate::reactive::RootScope::new();
    let handed = create_signal(false);
    let orphaned = create_signal(false);
    let (handed_body, orphaned_body) = (NodeRef::new(), NodeRef::new());
    let gone = NodeRef::new();
    let document = build_node(move || {
        provide_frame_anchor(create_memo(|| Some(TAB)));
        let tab = use_frame_anchor();
        view! {
            <List spacing=0.0>
                <Dialog
                    open={handed}
                    title="Text settings"
                    anchor={create_memo(move || DialogAnchor::Rect(tab.get()))}
                    width={Size::Custom(200.0)}
                    on_dismiss={move || handed.set(false)}
                >
                    <Frame @node_ref=&handed_body height=40.0 />
                </Dialog>
                <Dialog
                    open={orphaned}
                    title="Inspect"
                    anchor={DialogAnchor::Control(gone)}
                    width={Size::Custom(200.0)}
                    on_dismiss={move || orphaned.set(false)}
                >
                    <Frame @node_ref=&orphaned_body height=40.0 />
                </Dialog>
            </List>
        }
    });
    let mut harness = Harness::sized(document, WIDE_VIEWPORT);
    harness.frame(Vec::new());
    for (open, body) in [(handed, handed_body), (orphaned, orphaned_body)] {
        open.set(true);
        harness.settle();
        let rect = harness.rect(body.get());
        assert!(
            rect.top() > TAB.bottom() && rect.top() < TAB.bottom() + 60.0,
            "the card opens just under the tab: {rect:?} under {TAB:?}"
        );
        assert!(
            (rect.center().x - TAB.center().x).abs() < 2.0,
            "centred on it: {rect:?} on {TAB:?}"
        );
        open.set(false);
        harness.settle();
    }
}

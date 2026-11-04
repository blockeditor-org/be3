use super::*;
use crate::primitives::*;
use crate::reactive::*;

const PANEL_HEIGHT: f32 = 200.0;

#[test]
fn a_skipped_element_keeps_the_repaint_deadline_it_asked_for() {
    let _root = crate::reactive::RootScope::new();
    let (panel, text) = (NodeRef::new(), NodeRef::new());
    let mut document = build_node(move || {
        view! {
            <List spacing=0.0>
                <Frame @node_ref=&panel height=PANEL_HEIGHT fill=Color32::WHITE radius=0.0 />
                <Frame width=40.0 height=20.0>
                    <Drawing @node_ref=&text draw={blinking()} />
                </Frame>
            </List>
        }
    });
    let (panel, text) = (kind_of::<FrameNode>(&document, panel.get()), text.get());
    let (_, paints) = counted(&mut document, text);
    let mut harness = Harness::new(document);
    harness.frame(Vec::new());
    let settled = paints.get_untracked();

    harness
        .document_mut()
        .set_frame_color(panel, Color32::from_gray(90));
    let output = harness.frame(Vec::new());

    assert_eq!(
        paints.get_untracked(),
        settled,
        "the drawing is outside the damage"
    );
    assert!(
        output.repaint_after < Duration::MAX,
        "a skipped element must still ask for the repaint it was waiting on"
    );
}

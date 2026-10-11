use super::*;
use crate::{ChildBlock, ChildState, ChildTarget};
use beui::reactive::{Canvas, CanvasItem, Frame, Interactive, NodeRef, view};
use block_plugin_api::ChildMode;

const BLOCK: Uuid = Uuid::from_u128(1);
const BLOCK_TYPE: Uuid = Uuid::from_u128(2);
const CHILD: f32 = 200.0;
const GRIP: f32 = 12.0;

struct GrippedApp;

impl crate::BeuiApp for GrippedApp {
    fn view(editor: crate::Editor) -> beui::NodeId {
        let child = NodeRef::new();
        view! {
            <Frame width=CHILD height=CHILD>
                <Canvas>
                    <CanvasItem x=0.0 y=0.0 width=CHILD height=CHILD>
                        <ChildBlock
                            @node_ref={&child}
                            editor={editor}
                            block={Some(ChildTarget::new(BLOCK, BLOCK_TYPE))}
                            mode=ChildMode::Live
                            own_frame=true
                            on_state={move |_: ChildState| {}}
                        />
                    </CanvasItem>
                    <CanvasItem x={CHILD - GRIP} y={CHILD - GRIP} width=GRIP height=GRIP>
                        <Interactive capture_presses=true>
                            <Frame />
                        </Interactive>
                    </CanvasItem>
                </Canvas>
            </Frame>
        }
    }
}

#[test]
fn a_control_drawn_over_a_child_is_withheld_from_the_child() {
    let mut session = session::<GrippedApp>(BLOCK_TYPE);
    frame(&mut session, None, TopBar::Hidden);

    session.run(EditorRegion::Frame, 1, 0);
    session.run(EditorRegion::Frame, 2, 0);

    let children = session.placed_children(EditorRegion::Frame).len();
    assert_eq!(children, 1, "the child is placed");
    let occluder = session
        .occluders(EditorRegion::Frame)
        .iter()
        .find(|occluder| occluder.after as usize >= children)
        .expect("a control drawn over the child must keep its presses from the child");
    assert_eq!(
        (occluder.rect.width, occluder.rect.height),
        (GRIP, GRIP),
        "only the control's own rectangle is withheld"
    );
}

use super::*;
use crate::{ChildBlock, ChildState, ChildTarget};
use beui::reactive::{Frame, view};
use block_plugin_api::ChildMode;

const SLIDE: Uuid = Uuid::from_u128(1);
const SLIDE_TYPE: Uuid = Uuid::from_u128(2);

struct NestingApp;

impl crate::BeuiApp for NestingApp {
    fn view(editor: crate::Editor) -> beui::NodeId {
        view! {
            <Frame>
                <ChildBlock
                    editor={editor}
                    block={Some(ChildTarget::new(SLIDE, SLIDE_TYPE))}
                    mode=ChildMode::Live
                    own_frame=true
                    on_state={move |_: ChildState| {}}
                />
            </Frame>
        }
    }
}

#[test]
fn a_child_block_asks_the_host_for_the_frame_it_will_own() {
    let mut session = session::<NestingApp>(Uuid::new_v4());
    frame(&mut session, None, false);

    session.run(EditorRegion::Frame, 1);
    session.run(EditorRegion::Frame, 2);

    let children = session.placed_children(EditorRegion::Frame);
    let child = children.first().expect("the app placed no child block");
    assert_eq!(child.block_id, SLIDE.into_bytes());
    assert_eq!(child.mode, ChildMode::Live);
    assert!(
        child.own_frame,
        "a child block asking to own its frame should say so to the host"
    );
}

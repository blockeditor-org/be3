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
    let mut session = EditorSession::new::<NestingApp>(EditorInstanceId(0), Waker::default());
    session.connect(Uuid::new_v4(), Uuid::new_v4());
    session.regions.insert(
        EditorRegion::Frame,
        RegionState {
            placement: Some(ScreenPlacement {
                screen: block_plugin_api::ScreenId(0),
                instance: EditorInstanceId(0),
                region: EditorRegion::Frame,
                x: 0,
                y: 0,
                width: 800,
                height: 600,
                scale_factor_millis: 1000,
            }),
            metrics: Some(ViewportMetrics {
                logical_width: 800.0,
                logical_height: 600.0,
                visible_x: 0.0,
                visible_y: 0.0,
                pixel_width: 800,
                pixel_height: 600,
                scale_factor: 1.0,
            }),
            frame: Some(FrameSpec {
                chrome: FrameChrome::Drawn,
                content: None,
                top_bar: false,
            }),
            ..Default::default()
        },
    );

    session.run(EditorRegion::Frame, 1);
    session.run(EditorRegion::Frame, 2);

    let children = &session
        .regions
        .get(&EditorRegion::Frame)
        .expect("the frame region was set up")
        .children;
    let child = children.first().expect("the app placed no child block");
    assert_eq!(child.block_id, SLIDE.into_bytes());
    assert_eq!(child.mode, ChildMode::Live);
    assert!(
        child.own_frame,
        "a child block asking to own its frame should say so to the host"
    );
}

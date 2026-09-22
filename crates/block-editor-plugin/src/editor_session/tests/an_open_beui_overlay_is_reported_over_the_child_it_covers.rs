use super::*;
use crate::{ChildBlock, ChildState, ChildTarget};
use beui::ItemSize;
use beui::reactive::{Frame, List, NodeRef, view};
use beui::unstyled::Floating;
use block_plugin_api::ChildMode;

const BLOCK: Uuid = Uuid::from_u128(1);
const BLOCK_TYPE: Uuid = Uuid::from_u128(2);
const PANEL: f32 = 120.0;

struct OverlaidApp;

impl crate::BeuiApp for OverlaidApp {
    fn view(editor: crate::Editor) -> beui::NodeId {
        let child = NodeRef::new();
        view! {
            <Frame>
                <List spacing=0.0>
                    <ChildBlock
                        @node_ref={&child}
                        @sizing=ItemSize::Percent(100.0)
                        editor={editor}
                        block={Some(ChildTarget::new(BLOCK, BLOCK_TYPE))}
                        mode=ChildMode::Live
                        own_frame=true
                        on_state={move |_: ChildState| {}}
                    />
                    <Floating anchor={child}>
                        <Frame width=PANEL height=PANEL />
                    </Floating>
                </List>
            </Frame>
        }
    }
}

#[test]
fn an_open_beui_overlay_is_reported_over_the_child_it_covers() {
    let mut session = EditorSession::beui::<OverlaidApp>(
        Rc::new(Vec::new()),
        EditorInstanceId(0),
        Waker::default(),
    );
    let client = Arc::new(BlockClient::new(Uuid::new_v4(), Uuid::new_v4()));
    session.connect(client, Uuid::new_v4(), BLOCK_TYPE);
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
                trail: Vec::new(),
            }),
            ..Default::default()
        },
    );

    session.run_beui(EditorRegion::Frame, 1);
    session.run_beui(EditorRegion::Frame, 2);

    let state = session
        .regions
        .get(&EditorRegion::Frame)
        .expect("the frame region was set up");
    let report = state.report.as_ref().expect("the frame region reports");
    let floating = report
        .floating
        .first()
        .expect("an open overlay must be reported so the host blits it over the children");
    assert_eq!(
        (floating.width, floating.height),
        (PANEL, PANEL),
        "the reported rectangle is the one the overlay was laid out at"
    );
    let occluder = state
        .occluders
        .first()
        .expect("an open overlay must be withheld from the child under it");
    assert_eq!(
        (occluder.rect.width, occluder.rect.height),
        (PANEL, PANEL),
        "the occluder covers the same rectangle the overlay was laid out at"
    );
    assert!(
        occluder.after as usize >= state.children.len(),
        "the overlay occludes every child placed under it"
    );
}

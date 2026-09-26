use super::*;
use block_editor_plugin::editor_session::EditorSession;
use block_editor_plugin::{Plugin, Waker};
use block_plugin_api::{
    ChildRect, EditorInstanceId, FrameChrome, FrameSpec, ScreenId, ScreenPlacement, ScreenRequest,
    ViewportMetrics,
};

mod a_child_block_asks_the_host_for_the_frame_it_will_own;
mod a_docked_editor_hands_its_tabs_to_the_host_as_panes;
mod a_focused_beui_child_gets_the_whole_frame_not_just_its_embedded_rect;
mod a_focused_beui_childs_reported_content_is_its_own_canvas_not_the_whole_view;
mod a_layout_the_host_rearranged_comes_back_as_the_editors_own;
mod a_visible_editor_shows_its_user_as_active;
mod an_open_beui_overlay_is_reported_over_the_child_it_covers;

fn session<A: BeuiApp>(block_type: Uuid) -> EditorSession {
    let mut session =
        EditorSession::new(EditorInstanceId(0), Waker::default(), BeuiPlugin::<A>::open);
    session.connect(Uuid::new_v4(), block_type);
    session
}

fn frame(session: &mut EditorSession, content: Option<ChildRect>, top_bar: bool) {
    session.place(
        &[ScreenPlacement {
            screen: ScreenId(0),
            instance: EditorInstanceId(0),
            region: EditorRegion::Frame,
            x: 0,
            y: 0,
            width: 800,
            height: 600,
            scale_factor_millis: 1000,
        }],
        &[ScreenRequest {
            screen: ScreenId(0),
            instance: EditorInstanceId(0),
            region: EditorRegion::Frame,
            metrics: ViewportMetrics {
                logical_width: 800.0,
                logical_height: 600.0,
                visible_x: 0.0,
                visible_y: 0.0,
                pixel_width: 800,
                pixel_height: 600,
                scale_factor: 1.0,
            },
            frame: Some(FrameSpec {
                chrome: FrameChrome::Drawn,
                content,
                top_bar,
            }),
        }],
    );
}

fn regions(session: &mut EditorSession, regions: &[EditorRegion]) {
    let placements: Vec<ScreenPlacement> = regions
        .iter()
        .enumerate()
        .map(|(index, region)| ScreenPlacement {
            screen: ScreenId(index as u64),
            instance: EditorInstanceId(0),
            region: *region,
            x: 0,
            y: 300 * index as u32,
            width: 400,
            height: 300,
            scale_factor_millis: 1000,
        })
        .collect();
    let requests: Vec<ScreenRequest> = regions
        .iter()
        .enumerate()
        .map(|(index, region)| ScreenRequest {
            screen: ScreenId(index as u64),
            instance: EditorInstanceId(0),
            region: *region,
            metrics: ViewportMetrics {
                logical_width: 400.0,
                logical_height: 300.0,
                visible_x: 0.0,
                visible_y: 0.0,
                pixel_width: 400,
                pixel_height: 300,
                scale_factor: 1.0,
            },
            frame: Some(FrameSpec::default()),
        })
        .collect();
    session.place(&placements, &requests);
}

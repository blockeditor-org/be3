use super::*;
use block_editor_plugin::editor_session::EditorSession;
use block_editor_plugin::{Plugin, Waker};
use block_plugin_api::{
    EditorInstanceId, FrameChrome, FrameSpec, Rect, ScreenId, ScreenPlacement, ScreenRequest,
    TopBar, ViewportMetrics,
};

mod a_child_block_asks_the_host_for_the_frame_it_will_own;
mod a_focused_beui_child_gets_the_whole_frame_not_just_its_embedded_rect;
mod a_focused_beui_childs_reported_content_is_its_own_canvas_not_the_whole_view;
mod a_visible_editor_shows_its_user_as_active;
mod an_open_beui_overlay_is_reported_over_the_child_it_covers;
mod the_frame_and_the_preview_of_one_block_both_load_its_content;

fn session<A: BeuiApp>(block_type: Uuid) -> EditorSession {
    let mut session =
        EditorSession::new(EditorInstanceId(0), Waker::default(), BeuiPlugin::<A>::open);
    session.connect(Uuid::new_v4(), block_type);
    session
}

fn frame(session: &mut EditorSession, content: Option<Rect>, top_bar: TopBar) {
    session.place(
        &[ScreenPlacement {
            screen: ScreenId(0),
            instance: EditorInstanceId(0),
            region: EditorRegion::Frame,
            surface: 1,
            width: 800,
            height: 600,
            scale_factor_millis: 1000,
            raster_millis: 1000,
        }],
        &[ScreenRequest {
            screen: ScreenId(0),
            instance: EditorInstanceId(0),
            region: EditorRegion::Frame,
            metrics: ViewportMetrics {
                logical: block_plugin_api::Size {
                    width: 800.0,
                    height: 600.0,
                },
                visible: block_plugin_api::Pos2::ZERO,
                pixel_width: 800,
                pixel_height: 600,
                scale_factor: 1.0,
                raster: 1.0,
            },
            frame: Some(FrameSpec {
                chrome: FrameChrome::Drawn,
                content,
                top_bar,
            }),
            windows: false,
            placement: None,
        }],
    );
}

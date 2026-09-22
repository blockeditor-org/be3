use super::*;
use block_plugin_api::{
    DEFAULT_SURFACE_SIDE, EditorRegion, HelloAccepted, InputBatch, ScreenRequest, ScreenSet, Size,
    SurfaceFormat, SurfaceSpec, Theme, TunnelMessage, ViewportMetrics,
};

fn accept(session: &mut ClientSession) {
    session.receive(Message::HelloAccepted(HelloAccepted {
        version: PROTOCOL_VERSION,
        host_name: "test host".into(),
        surface: Some(SurfaceSpec {
            format: SurfaceFormat::Rgba8Unorm,
            max_side: DEFAULT_SURFACE_SIDE,
        }),
        theme: Theme { dark: true },
    }));
}

fn open(session: &mut ClientSession, instance: EditorInstanceId) {
    session.receive(Message::Editor(block_plugin_api::EditorMessage::Open {
        instance,
        block_id: [1; 16],
        block_type: [2; 16],
        account_id: [3; 16],
        workspace_id: [4; 16],
        client_id: [5; 16],
        editable: true,
    }));
}

fn screen(screen: ScreenId, instance: EditorInstanceId) -> ScreenRequest {
    ScreenRequest {
        screen,
        instance,
        region: EditorRegion::Frame,
        frame: None,
        metrics: ViewportMetrics {
            logical_width: 100.0,
            logical_height: 100.0,
            visible_x: 0.0,
            visible_y: 0.0,
            pixel_width: 100,
            pixel_height: 100,
            scale_factor: 1.0,
        },
    }
}

mod accepts_a_theme_change_while_running;
mod accepts_client_responses_after_the_last_instance_closes;
mod accepts_content_for_an_open_instance;
mod accepts_ordered_lifecycle;
mod opens_and_closes_editor_instance;
mod refuses_to_draw_without_a_surface;
mod rejects_child_statuses_for_unopened_instances;
mod rejects_messages_only_a_plugin_may_send;
mod rejects_out_of_order_messages;
mod rejects_screens_for_unopened_instances;

fn content(instance: EditorInstanceId) -> Message {
    Message::Editor(block_plugin_api::EditorMessage::Content {
        instance,
        content_type: [7; 16],
        bytes: vec![0; 8],
        applied: 0,
    })
}
mod rejects_content_for_an_instance_that_was_never_opened;

use super::*;
use std::collections::HashSet;

fn hello() -> Message {
    Message::Hello(Hello {
        version: PROTOCOL_VERSION,
        plugin: PluginIdentity {
            id: "demo".into(),
            name: "Plugin Demo".into(),
            version: "1.0".into(),
        },
        surface: SurfaceSupport::Texture,
    })
}

fn request(instance: u64, request_id: u64, request: HostRequest) -> Message {
    Message::Editor(EditorMessage::Request {
        instance: EditorInstanceId(instance),
        request_id,
        request,
    })
}

fn reply(instance: u64, request_id: u64, reply: HostReply) -> Message {
    Message::Editor(EditorMessage::Replied {
        instance: EditorInstanceId(instance),
        request_id,
        reply,
    })
}

fn screen(
    screen: u64,
    instance: u64,
    pixel_width: u32,
    pixel_height: u32,
    scale_factor: f32,
) -> ScreenRequest {
    region_screen(
        EditorRegion::Frame,
        screen,
        instance,
        pixel_width,
        pixel_height,
        scale_factor,
    )
}

fn region_screen(
    region: EditorRegion,
    screen: u64,
    instance: u64,
    pixel_width: u32,
    pixel_height: u32,
    scale_factor: f32,
) -> ScreenRequest {
    ScreenRequest {
        screen: ScreenId(screen),
        instance: EditorInstanceId(instance),
        region,
        frame: None,
        metrics: ViewportMetrics {
            logical_width: pixel_width as f32 / scale_factor,
            logical_height: pixel_height as f32 / scale_factor,
            visible_x: 0.0,
            visible_y: 0.0,
            pixel_width,
            pixel_height,
            scale_factor,
        },
    }
}

mod a_layout_gives_each_shown_screen_its_own_surface;
mod a_paste_over_the_text_limit_arrives_in_pieces;
mod artifact_messages_round_trip;
mod artifact_watch_messages_round_trip;
mod audio_messages_round_trip;
mod bar_actions_round_trip;
mod block_commands_round_trip;
mod block_content_and_operations_round_trip;
mod block_types_round_trip;
mod child_placements_round_trip;
mod child_statuses_round_trip;
mod child_view_changes_round_trip;
mod clipboard_messages_round_trip;
mod copied_text_round_trips;
mod creation_messages_round_trip;
mod cursor_round_trips;
mod data_messages_round_trip;
mod drag_messages_round_trip;
mod every_block_id_a_message_carries_is_visited;
mod every_editor_manifest_parses;
mod every_key_round_trips;
mod fetch_messages_round_trip;
mod file_drop_messages_round_trip;
mod file_pick_messages_round_trip;
mod file_save_messages_round_trip;
mod focus_messages_round_trip;
mod forwarded_picks_and_creation_children_round_trip;
mod frame_round_trips;
mod frame_screens_and_reports_round_trip;
mod grabbing_the_cursor_round_trips;
mod history_messages_round_trip;
mod host_panel_messages_round_trip;
mod host_window_messages_round_trip;
mod ime_messages_round_trip;
mod manifest_validation;
mod menus_and_their_picks_round_trip;
mod multiplexed_messages_round_trip;
mod open_block_request_round_trips;
mod open_messages_round_trip;
mod performance_messages_round_trip;
mod pick_block_messages_round_trip;
mod power_messages_round_trip;
mod presence_messages_round_trip;
mod present_messages_round_trip;
mod region_sizes_round_trip;
mod rejects_artifact_settings_over_limit;
mod rejects_collection_over_limit;
mod rejects_malformed_payload;
mod rejects_truncated_frame;
mod rejects_unknown_message_kind;
mod rejects_unordered_occluders;
mod replacing_a_child_round_trips;
mod resize_messages_round_trip;
mod shell_dialog_and_access_messages_round_trip;
mod show_block_request_round_trips;
mod theme_messages_round_trip;
mod touch_input_round_trips;
mod version_control_messages_round_trip;
mod view_messages_round_trip;
mod web_view_messages_round_trip;
mod zoom_gesture_round_trips;

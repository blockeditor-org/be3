use super::*;
use std::collections::HashSet;

fn hello() -> Message {
    Message::Hello(Hello {
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

fn frame(serial: u64) -> FrameCommit {
    FrameCommit {
        serial,
        sizes: Vec::new(),
        reports: Vec::new(),
        windows: Vec::new(),
        children: Vec::new(),
        repaint_after_micros: None,
        animating: false,
        presented: None,
    }
}

fn committed(children: ChildPlacements) -> Message {
    Message::Frame(FrameCommit {
        children: vec![children],
        ..frame(1)
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
            logical: Size {
                width: pixel_width as f32 / scale_factor,
                height: pixel_height as f32 / scale_factor,
            },
            visible: Pos2::ZERO,
            pixel_width,
            pixel_height,
            scale_factor,
            monitors: Vec::new(),
        },
    }
}

mod a_frame_says_which_host_state_it_was_laid_out_against;
mod a_layout_gives_each_shown_screen_its_own_surface;
mod a_menu_nested_too_deeply_is_refused_while_decoding;
mod a_paste_over_the_text_limit_arrives_in_pieces;
mod a_screen_readers_action_round_trips_and_a_huge_tree_is_refused;
mod an_intercepted_key_round_trips;
mod artifact_messages_round_trip;
mod artifact_watch_messages_round_trip;
mod audio_messages_round_trip;
mod block_commands_round_trip;
mod block_content_and_operations_round_trip;
mod block_types_round_trip;
mod child_placements_round_trip;
mod child_statuses_round_trip;
mod creation_messages_round_trip;
mod data_messages_round_trip;
mod epaper_messages_round_trip;
mod every_block_id_a_message_carries_is_visited;
mod every_editor_manifest_parses;
mod every_key_round_trips;
mod fetch_messages_round_trip;
mod file_pick_messages_round_trip;
mod file_save_messages_round_trip;
mod forwarded_picks_and_creation_children_round_trip;
mod frame_round_trips;
mod frame_screens_and_reports_round_trip;
mod head_messages_round_trip;
mod history_messages_round_trip;
mod host_panel_messages_round_trip;
mod host_window_messages_round_trip;
mod ime_messages_round_trip;
mod manifest_validation;
mod media_messages_round_trip;
mod menus_and_their_picks_round_trip;
mod multiplexed_messages_round_trip;
mod notification_messages_round_trip;
mod open_block_request_round_trips;
mod open_messages_round_trip;
mod pick_block_messages_round_trip;
mod pointer_motion_round_trips;
mod power_messages_round_trip;
mod preference_messages_round_trip;
mod region_sizes_round_trip;
mod rejects_artifact_settings_over_limit;
mod rejects_collection_over_limit;
mod rejects_malformed_payload;
mod rejects_truncated_frame;
mod rejects_unknown_message_kind;
mod rejects_unordered_occluders;
mod replacing_a_child_round_trips;
mod replica_frames_round_trip;
mod shell_dialog_and_access_messages_round_trip;
mod show_block_request_round_trips;
mod touch_input_round_trips;
mod tray_messages_round_trip;
mod version_control_messages_round_trip;
mod web_view_messages_round_trip;
mod wifi_messages_round_trip;
mod window_requests_round_trip;
mod zoom_gesture_round_trips;

fn host_value_round_trips<T: HostValue>(value: T::Value)
where
    T::Value: std::fmt::Debug,
{
    let watch = Message::Editor(EditorMessage::WatchHostValue {
        instance: EditorInstanceId(4),
        key: T::KEY.into(),
    });
    assert_eq!(watch.direction(), Direction::ToHost);
    assert_eq!(decode_frame(&encode_frame(&watch).unwrap()).unwrap(), watch);
    let message = Message::Editor(EditorMessage::HostValue {
        instance: EditorInstanceId(4),
        key: T::KEY.into(),
        value: encode_host(&value),
    });
    assert_eq!(message.direction(), Direction::ToPlugin);
    let Message::Editor(EditorMessage::HostValue { value: bytes, .. }) =
        decode_frame(&encode_frame(&message).unwrap()).unwrap()
    else {
        panic!("a host value comes back as one");
    };
    assert_eq!(decode_host::<T::Value>(&bytes), Some(value));
}

fn host_action_round_trips<A: HostAction + PartialEq + std::fmt::Debug>(action: A) {
    let message = Message::Editor(EditorMessage::HostAction {
        instance: EditorInstanceId(4),
        key: A::KEY.into(),
        action: encode_host(&action),
    });
    assert_eq!(message.direction(), Direction::ToHost);
    let Message::Editor(EditorMessage::HostAction { action: bytes, .. }) =
        decode_frame(&encode_frame(&message).unwrap()).unwrap()
    else {
        panic!("a host action comes back as one");
    };
    assert_eq!(decode_host::<A>(&bytes), Some(action));
}

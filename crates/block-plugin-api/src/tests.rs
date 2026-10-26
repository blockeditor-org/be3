use super::*;
use std::collections::HashSet;

fn round_trips<T>(value: &T)
where
    T: Serialize + serde::de::DeserializeOwned + PartialEq + std::fmt::Debug,
{
    assert_eq!(
        crate::ops::decode::<T>(&crate::ops::encode(value)).as_ref(),
        Some(value)
    );
}

fn hello() -> Hello {
    Hello {
        plugin: PluginIdentity {
            id: "demo".into(),
            name: "Plugin Demo".into(),
            version: "1.0".into(),
        },
        surface: SurfaceSupport::Texture,
    }
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
        window_requests: Vec::new(),
    }
}

fn committed(children: ChildPlacements) -> FrameCommit {
    FrameCommit {
        children: vec![children],
        ..frame(1)
    }
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
            raster: scale_factor,
        },
        windows: false,
        placement: None,
    }
}

mod a_description_keeps_every_kind_of_node_property;
mod a_frame_says_which_host_state_it_was_laid_out_against;
mod a_layout_gives_each_shown_screen_its_own_surface;
mod a_manifest_grants_only_extensions_the_host_knows;
mod a_menu_nested_too_deeply_is_refused_while_nesting;
mod a_paste_over_the_text_limit_arrives_in_pieces;
mod a_screen_readers_action_round_trips;
mod a_show_request_already_waiting_moves_to_the_end;
mod a_watch_answers_its_next_with_the_latest_change;
mod an_artifact_settings_child_round_trips;
mod an_intercepted_key_round_trips;
mod artifact_messages_round_trip;
mod block_types_round_trip;
mod child_placements_round_trip;
mod child_statuses_round_trip;
mod creation_children_round_trip;
mod creation_messages_round_trip;
mod every_block_id_a_message_carries_is_visited;
mod every_editor_manifest_parses;
mod every_key_round_trips;
mod frame_round_trips;
mod frame_screens_and_reports_round_trip;
mod host_panel_messages_round_trip;
mod host_window_messages_round_trip;
mod ime_messages_round_trip;
mod manifest_validation;
mod media_messages_round_trip;
mod menus_and_their_picks_round_trip;
mod notification_messages_round_trip;
mod open_messages_round_trip;
mod pointer_motion_round_trips;
mod power_messages_round_trip;
mod preference_messages_round_trip;
mod region_sizes_round_trip;
mod rejects_unordered_occluders;
mod touch_input_round_trips;
mod tray_messages_round_trip;
mod web_view_events_waiting_for_the_editor_fold_and_stay_bounded;
mod wifi_messages_round_trip;
mod window_requests_round_trip;
mod zoom_gesture_round_trips;

fn watched_round_trips<W: Watched>(value: W::Value)
where
    W::Value: std::fmt::Debug,
{
    round_trips(&value);
}

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
        docks: Vec::new(),
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
        docked: false,
    }
}

mod a_childs_dock_splices_in_place_of_its_tab_and_splits_back_out;
mod a_description_keeps_every_kind_of_node_property;
mod a_dock_declaration_round_trips_and_nests;
mod a_frame_says_which_host_state_it_was_laid_out_against;
mod a_hosted_state_waiting_for_the_editor_is_replaced_by_the_latest;
mod a_layout_gives_each_shown_screen_its_own_surface;
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
mod every_key_round_trips;
mod frame_round_trips;
mod frame_screens_and_reports_round_trip;
mod host_panel_messages_round_trip;
mod host_window_messages_round_trip;
mod ime_messages_round_trip;
mod insets_give_the_content_area_of_a_region_resized_since_they_were_reported;
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

fn pane(depth: u32, key: &str, locked: bool) -> DockItem {
    DockItem {
        depth,
        kind: DockItemKind::Pane(DockPane {
            key: key.into(),
            active: Some(0),
            vertical: false,
            icons_only: false,
            sidebar: 180.0,
            keep: false,
            area: locked.then(|| key.into()),
            empty: None,
        }),
    }
}

fn tab(depth: u32, path: DockTabPath) -> DockItem {
    DockItem {
        depth,
        kind: DockItemKind::Tab(path),
    }
}

fn declared(path: DockTabPath, key: &str) -> DockTab {
    DockTab {
        tab: path,
        key: key.into(),
        title: key.into(),
        glyph: String::new(),
        closable: true,
        menu: Vec::new(),
    }
}

fn declaration() -> DockDeclaration {
    DockDeclaration {
        tree: DockTree {
            items: vec![
                DockItem {
                    depth: 0,
                    kind: DockItemKind::Split(DockSplit {
                        key: "main".into(),
                        direction: DockDirection::Horizontal,
                        fraction: 0.25,
                    }),
                },
                pane(1, "files", true),
                tab(2, DockTabPath::own(1)),
                pane(1, "editors", false),
                tab(2, DockTabPath::own(2)),
                tab(2, DockTabPath::own(3)),
            ],
            focus: Some(DockTabPath::own(2)),
        },
        tabs: vec![
            declared(DockTabPath::own(1), "files"),
            declared(DockTabPath::own(2), "editor"),
            declared(DockTabPath::own(3), "nested"),
        ],
        dialogs: Vec::new(),
    }
}

fn child_declaration() -> DockDeclaration {
    DockDeclaration {
        tree: DockTree {
            items: vec![pane(0, "side", true), tab(1, DockTabPath::own(4))],
            focus: None,
        },
        tabs: vec![declared(DockTabPath::own(4), "side-tab")],
        dialogs: Vec::new(),
    }
}

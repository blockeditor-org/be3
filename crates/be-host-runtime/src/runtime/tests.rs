use super::*;
use be_sdk_api::{ChildPlacements, DockMessage, PresentedFrame};
mod a_child_comes_or_goes_only_with_the_frame_its_placer_draws_it_in;
mod a_failed_app_session_is_reported_as_a_crash;
mod a_frame_held_for_its_children_is_dropped_when_the_app_stops_or_restarts;
mod a_frame_whose_children_wait_for_the_poll_is_shown_with_them;
mod a_frames_dock_messages_wait_with_the_children_it_places;
mod a_panic_while_the_host_runs_an_app_stops_only_that_app;
mod a_preview_shown_twice_is_drawn_whole_at_its_larger_place;
mod a_stopped_apps_view_keeps_the_place_it_is_shown_at;
mod a_view_drawn_finer_than_its_window_is_sent_its_raster_beside_its_scale;
mod a_view_hears_where_its_anchored_views_go_and_one_can_block_it_or_take_its_keyboard;
mod a_view_resized_before_its_app_draws_the_new_size_keeps_its_last_frame_unstretched;
mod a_view_shown_again_draws_its_last_frame_until_its_app_draws_it_anew;
mod an_anchored_view_that_closes_stays_until_the_frame_that_closed_it_is_drawn;
mod an_app_dropped_while_running_is_heard_to_stop;
mod an_app_not_running_yet_hears_only_where_the_keyboard_is;
mod an_app_frame_that_changes_nothing_the_host_draws_leaves_the_host_with_nothing_to_redo;
mod an_app_s_calls_are_served_in_the_order_it_made_them;
mod an_app_with_no_editors_open_is_stopped_once_it_has_been_unused_for_a_while;
mod children_that_move_wait_with_a_child_that_comes_in_the_same_layout;
mod killing_an_app_stops_it_as_any_stop_does;
mod only_the_views_hidden_last_keep_their_last_frame;

fn manifest() -> AppManifest {
    be_sdk_manifest::manifest_from_json(
        r#"{
            "id": "test.switcher",
            "name": "Switcher",
            "version": "0.1.0",
            "editors": {"main": {
                "id": "00000000-0000-4000-8000-00000000e41d",
                "edits": "00000000-0000-4000-8000-0000000000d4",
                "display_name": "Switcher",
                "icon": "x",
                "views": ["Main"]
            }},
            "entry_point": "test.switcher.wasm"
        }"#,
    )
    .expect("the manifest is valid")
}
fn placing(serial: u64, instances: &[u64]) -> Heard {
    Heard::LaidOut(Box::new(LayoutCommit {
        serial,
        sizes: Vec::new(),
        reports: Vec::new(),
        windows: Vec::new(),
        children: instances
            .iter()
            .map(|instance| ChildPlacements {
                instance: EditorInstanceId(*instance),
                view: ViewId::MAIN,
                children: Vec::new(),
                occluders: Vec::new(),
            })
            .collect(),
        window_requests: Vec::new(),
        docks: Vec::new(),
    }))
}
fn presented(serial: u64, sequence: u64) -> Heard {
    Heard::Presented(PresentCommit {
        serial,
        repaint_after_micros: None,
        animating: false,
        input_waiting: false,
        presented: Some(PresentedFrame {
            sequence,
            damage: Vec::new(),
            painted: Vec::new(),
        }),
    })
}
fn frame(presents: u64) -> Frame {
    Frame {
        textures: Vec::new(),
        presents,
        damage: None,
    }
}
fn unpainted(serial: u64) -> Heard {
    Heard::Presented(PresentCommit {
        serial,
        repaint_after_micros: None,
        animating: false,
        input_waiting: false,
        presented: None,
    })
}

use super::*;
use crate::plugin_host::EditorBlock;
use beui::Pos2;
use block_plugin_api::InputEvent;

const INSTANCE: EditorInstanceId = EditorInstanceId(1);
const REGION: EditorRegion = EditorRegion::Frame;
const PASS: u64 = 1;
const SIZE: Vec2 = vec2(100.0, 100.0);

fn placed() -> Instances {
    placed_on(Uuid::nil(), Uuid::nil())
}

fn placed_on(block: Uuid, block_type: Uuid) -> Instances {
    let rect = Rect::from_min_size(pos2(10.0, 10.0), SIZE);
    let mut instances = Instances::default();
    let block_types = Arc::new(block_plugin_api::Catalog::default());
    let role = InstanceRole::Editor(EditorBlock {
        id: block,
        block_type,
        view_block: None,
    });
    instances.report(
        INSTANCE,
        REGION,
        Uuid::nil(),
        role,
        &block_types,
        Some(block_plugin_api::FrameSpec::default()),
        SIZE,
        Rect::from_min_size(Pos2::ZERO, SIZE),
        1.0,
        PASS,
    );
    instances.place(
        INSTANCE,
        REGION,
        Placement {
            rect,
            clip: rect,
            pass: PASS,
        },
    );
    instances
}

fn forwarded(
    events: Vec<beui::Event>,
    pointer: Option<Pos2>,
    focused: bool,
) -> beui::ForwardedInput {
    let rect = Rect::from_min_size(pos2(10.0, 10.0), SIZE);
    beui::ForwardedInput {
        events,
        rect,
        hovered: pointer.is_some_and(|pointer| rect.contains(pointer)),
        focused,
        pointer,
        modifiers: beui::Modifiers::NONE,
    }
}

fn input_events(messages: &[Message]) -> Vec<InputEvent> {
    messages
        .iter()
        .filter_map(|message| match message {
            Message::Input(batch) => Some(batch.events.clone()),
            _ => None,
        })
        .flatten()
        .collect()
}

fn next_screens(instances: &mut Instances) -> NextScreens {
    instances.touch(&crate::be::take_touched());
    instances.next_screens(PASS)
}

mod a_block_is_named_after_its_content_until_someone_names_it;
mod a_click_outside_a_frame_child_hands_the_frame_back;
mod a_database_view_given_content_references_its_database;
mod a_frame_takeover_keeps_the_last_painting_where_it_was;
mod a_message_waits_for_the_instance_it_names_to_be_opened;
mod a_plugin_is_told_when_the_pointer_leaves_it;
mod a_plugin_reaches_only_the_hosts_its_manifest_names;
mod a_plugin_seeds_only_blocks_it_holds;
mod a_restarted_plugin_hears_its_childrens_statuses_again;
mod a_restarted_plugin_is_sent_its_content_and_blocks_again;
mod a_window_close_is_kept_for_the_host_to_take;
mod after_the_first_snapshot_an_editor_is_sent_operations;
mod an_editor_can_read_and_edit_a_block_it_watches;
mod an_editor_is_only_sent_messages_its_plugin_session_accepts;
mod an_instance_inside_a_checkout_speaks_in_its_local_ids;
mod an_instance_the_plugin_never_opened_is_not_closed;
mod an_instance_watching_a_blocks_history_is_told_when_it_changes;
mod an_instance_watching_input_devices_is_told_what_is_connected;
mod clearing_a_name_names_the_block_after_its_content_at_once;
mod input_is_withheld_from_screens_the_plugin_no_longer_has;
mod the_view_a_screen_is_given_carries_the_scale_it_is_shown_at;
mod the_windows_the_host_runs_reach_an_instance_again_after_a_restart;

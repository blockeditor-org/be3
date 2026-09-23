use super::*;
use crate::plugin_host::EditorBlock;
use beui::Pos2;
use block_plugin_api::InputEvent;

const INSTANCE: EditorInstanceId = EditorInstanceId(1);
const REGION: EditorRegion = EditorRegion::Frame;
const PASS: u64 = 1;
const SIZE: Vec2 = vec2(100.0, 100.0);
const TARGET: Target = Target {
    instance: INSTANCE,
    region: REGION,
};

fn placed() -> Instances {
    placed_on(Uuid::nil(), Uuid::nil())
}

fn placed_on(block: Uuid, block_type: Uuid) -> Instances {
    let client = Arc::new(BlockClient::new(Uuid::nil(), Uuid::nil()));
    placed_with(&client, block, block_type)
}

fn placed_with(client: &Arc<BlockClient>, block: Uuid, block_type: Uuid) -> Instances {
    let rect = Rect::from_min_size(pos2(10.0, 10.0), SIZE);
    let mut instances = Instances::default();
    let block_types = Arc::new(Vec::new());
    let role = InstanceRole::Editor(EditorBlock {
        id: block,
        block_type,
    });
    instances.report(
        INSTANCE,
        REGION,
        client,
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
            target: TARGET,
            rect,
            clip: rect,
            pass: PASS,
        },
    );
    host::register(TARGET, rect, rect, 0);
    instances
}

mod a_frame_childs_chrome_is_withheld_from_the_editor_it_covers;
mod a_frame_takeover_keeps_the_last_painting_where_it_was;
mod a_message_waits_for_the_instance_it_names_to_be_opened;
mod a_migrated_block_is_named_after_its_content_until_someone_names_it;
mod a_migrated_editor_is_only_sent_messages_its_plugin_session_accepts;
mod a_plugin_is_told_when_the_pointer_leaves_it;
mod a_plugin_reaches_only_the_hosts_its_manifest_names;
mod a_press_under_a_dialog_is_withheld_from_the_plugin;
mod after_the_first_snapshot_an_editor_is_sent_operations;
mod an_editor_can_read_and_edit_a_block_it_watches;
mod an_instance_the_plugin_never_opened_is_not_closed;
mod an_instance_watching_a_blocks_history_is_told_when_it_changes;
mod f6_moves_the_focus_to_the_next_plugin;
mod input_is_withheld_from_screens_the_plugin_no_longer_has;
mod the_view_a_screen_is_given_carries_the_scale_it_is_shown_at;

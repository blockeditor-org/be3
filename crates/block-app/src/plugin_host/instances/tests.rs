use super::*;
use crate::plugin_host::EditorBlock;
use block_plugin_api::InputEvent;

const INSTANCE: EditorInstanceId = EditorInstanceId(1);
const REGION: EditorRegion = EditorRegion::Frame;
const PASS: u64 = 1;
const SIZE: egui::Vec2 = egui::vec2(100.0, 100.0);

fn placed() -> (Instances, egui::Context, egui::Id) {
    placed_on(Uuid::nil(), Uuid::nil())
}

fn placed_on(block: Uuid, block_type: Uuid) -> (Instances, egui::Context, egui::Id) {
    let client = Arc::new(BlockClient::new(Uuid::nil(), Uuid::nil()));
    placed_with(&client, block, block_type)
}

fn placed_with(
    client: &Arc<BlockClient>,
    block: Uuid,
    block_type: Uuid,
) -> (Instances, egui::Context, egui::Id) {
    let context = egui::Context::default();
    let rect = egui::Rect::from_min_size(egui::pos2(10.0, 10.0), SIZE);
    let id = egui::Id::new("plugin screen");
    let _ = context.run_ui(egui::RawInput::default(), |ui| {
        ui.interact(rect, id, egui::Sense::click_and_drag());
    });
    let mut instances = Instances::default();
    let block_types = Arc::new(Vec::new());
    let role = InstanceRole::Editor(EditorBlock {
        id: block,
        block_type,
    });
    instances.report(
        INSTANCE,
        REGION,
        &context,
        client,
        Uuid::nil(),
        role,
        &block_types,
        Some(block_plugin_api::FrameSpec::default()),
        SIZE,
        egui::Rect::from_min_size(egui::Pos2::ZERO, SIZE),
        1.0,
        PASS,
    );
    instances.place(
        INSTANCE,
        REGION,
        Placement {
            id,
            rect,
            clip: rect,
            pass: PASS,
        },
    );
    (instances, context, id)
}

mod a_frame_childs_chrome_is_withheld_from_the_editor_it_covers;
mod a_frame_takeover_keeps_the_last_painting_where_it_was;
mod a_message_waits_for_the_instance_it_names_to_be_opened;
mod a_migrated_block_is_named_after_its_content_until_someone_names_it;
mod a_migrated_editor_is_only_sent_messages_its_plugin_session_accepts;
mod a_plugin_is_told_when_the_pointer_leaves_it;
mod a_plugin_reaches_only_the_hosts_its_manifest_names;
mod after_the_first_snapshot_an_editor_is_sent_operations;
mod an_instance_the_plugin_never_opened_is_not_closed;
mod an_instance_watching_a_blocks_history_is_told_when_it_changes;
mod f6_moves_the_focus_to_the_next_plugin;
mod input_is_withheld_from_screens_the_plugin_no_longer_has;
mod the_view_a_screen_is_given_carries_the_scale_it_is_shown_at;

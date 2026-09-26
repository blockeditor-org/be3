use std::collections::{BTreeMap, HashSet};

use block_editor_beui::be_block::CanvasContent;
use block_editor_beui::be_block::canvas::Canvas;
use block_editor_beui::be_block::canvas::{
    CanvasComponent, CanvasEntity, CanvasEntityKind, CanvasEntityStyle, CanvasPoint,
    CanvasPreviewRegion, CanvasTransform, InfiniteCanvasOperation,
};
use block_editor_beui::be_block::database::DatabaseValue;
use block_editor_beui::{BlockInfo, BlockParent, Editor, EditorHost};
use block_ui_test::{BeuiTest, ContentHarness};
use uuid::Uuid;

use crate::app::CanvasApp;
use crate::app::state::{attach_component, remove_component, set_component_value};
use crate::geometry::{ResizeHandle, entity_bounds, resize_entities_axis};

mod a_direct_editor_entity_draws_the_frame_it_reserves;
mod a_moved_entity_is_drawn_where_it_was_dropped;
mod a_typed_transform_value_is_one_edit;
mod attaching_component_fills_only_missing_selected_entities;
mod clicking_an_entity_selects_it_and_shows_its_handles;
mod dragging_a_transform_field_twice_keeps_the_first_drag;
mod dragging_any_transform_field_moves_the_entity;
mod dragging_with_the_rectangle_tool_adds_a_rectangle;
mod every_transform_field_previews_what_is_typed;
mod removing_component_deletes_its_values_from_all_selected_entities;
mod replacing_a_referenced_block_rewrites_the_entity;
mod resizing_an_editor_that_cannot_resize_scales_it;
mod rotating_with_the_handle_is_drawn;
mod selections_are_shared_with_peers_and_theirs_are_drawn;
mod setting_component_value_writes_the_same_value_to_all_selected_entities;
mod the_actions_menu_deletes_the_selection;
mod the_canvas_paints_the_entities_it_holds;
mod the_intrinsic_size_follows_the_preview_region;
mod the_preview_centres_the_region_it_was_given;
mod the_transform_fields_edit_the_selected_entity;
mod typing_a_transform_value_and_pressing_escape_edits_nothing;

fn entity(id: Uuid) -> CanvasEntity {
    CanvasEntity {
        id,
        transform: CanvasTransform::new(CanvasPoint::default(), CanvasPoint::new(10.0, 10.0), 0.0),
        kind: CanvasEntityKind::Rectangle,
        style: CanvasEntityStyle::default(),
        group_id: None,
        locked: false,
        components: Vec::new(),
    }
}

fn open(canvas: &Canvas, preview: bool, known: &[BlockInfo]) -> ContentHarness<CanvasApp> {
    let block = Uuid::new_v4();
    let host = EditorHost::default();
    host.set_editable(true);
    let editor = Editor::new(host.clone(), block);
    let test = match preview {
        true => BeuiTest::preview(editor),
        false => BeuiTest::new(editor),
    };
    let mut harness = ContentHarness::new(test.in_viewport(), host);
    for info in known {
        harness.store().add_block(info.clone());
    }
    harness.hold(None, CanvasContent::new(canvas));
    harness.run();
    harness.run();
    harness
}

fn editor(entities: &[CanvasEntity]) -> ContentHarness<CanvasApp> {
    open(&Canvas::with_entities(entities.to_vec(), None), false, &[])
}

fn apply(editor: &mut ContentHarness<CanvasApp>, operation: InfiniteCanvasOperation) {
    let edit = editor
        .content::<CanvasContent>(None)
        .root()
        .edit_for(&operation);
    editor.edit::<CanvasContent>(None, &edit);
}

fn entities(editor: &ContentHarness<CanvasApp>) -> Vec<CanvasEntity> {
    editor.content::<CanvasContent>(None).root().entities()
}

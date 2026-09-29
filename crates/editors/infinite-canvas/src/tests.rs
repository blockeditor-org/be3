use std::collections::{BTreeMap, HashSet};

use block_editor_beui::be_block::CanvasContent;
use block_editor_beui::be_block::canvas::Canvas;
use block_editor_beui::be_block::canvas::{
    CanvasComponent, CanvasEntity, CanvasEntityKind, CanvasEntityStyle, CanvasPoint,
    CanvasTransform, InfiniteCanvasOperation,
};
use block_editor_beui::be_block::database::DatabaseValue;
use block_editor_beui::beui::Vec2;
use block_editor_beui::{BlockInfo, BlockParent, Editor, EditorHost};
use block_ui_test::BeuiTest;
use uuid::Uuid;

use crate::app::CanvasApp;
use crate::app::state::{attach_component, remove_component, set_component_value};
use crate::geometry::{ResizeHandle, entity_bounds, resize_entities_axis};

mod a_direct_editor_entity_draws_the_frame_it_reserves;
mod a_finger_drags_a_transform_field_in_the_sheet;
mod a_finger_just_outside_a_corner_resizes_the_selection;
mod a_moved_entity_is_drawn_where_it_was_dropped;
mod a_narrow_canvas_opens_its_inspector_under_the_stage;
mod a_phone_canvas_keeps_its_tools_in_a_dock_and_zoom_pill;
mod a_second_finger_calls_off_the_shape_being_drawn;
mod a_typed_transform_value_is_one_edit;
mod an_image_still_loading_is_drawn_as_its_thumbhash;
mod attaching_component_fills_only_missing_selected_entities;
mod clicking_a_live_editor_hands_it_the_frame;
mod clicking_an_entity_selects_it_and_shows_its_handles;
mod dragging_a_transform_field_twice_keeps_the_first_drag;
mod dragging_any_transform_field_moves_the_entity;
mod dragging_with_the_artboard_tool_adds_an_artboard;
mod dragging_with_the_rectangle_tool_adds_a_rectangle;
mod drawing_with_a_finger_adds_a_rectangle;
mod every_transform_field_previews_what_is_typed;
mod removing_component_deletes_its_values_from_all_selected_entities;
mod replacing_a_referenced_block_rewrites_the_entity;
mod resizing_an_editor_that_cannot_resize_scales_it;
mod rotating_with_the_handle_is_drawn;
mod selections_are_shared_with_peers_and_theirs_are_drawn;
mod setting_component_value_writes_the_same_value_to_all_selected_entities;
mod the_actions_menu_deletes_the_selection;
mod the_canvas_opens_with_the_select_tool_so_a_finger_box_selects;
mod the_canvas_paints_the_entities_it_holds;
mod the_intrinsic_size_follows_the_first_artboard;
mod the_layers_panel_selects_reorders_and_hides;
mod the_preview_shows_the_first_artboard;
mod tapping_selected_text_on_a_phone_edits_it;
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

fn artboard(id: u128, center: CanvasPoint, size: CanvasPoint) -> CanvasEntity {
    CanvasEntity {
        id: Uuid::from_u128(id),
        transform: CanvasTransform::new(center, size, 0.0),
        kind: CanvasEntityKind::Artboard {
            name: format!("Artboard {id}"),
        },
        style: CanvasEntityStyle::default(),
        group_id: None,
        locked: false,
        components: Vec::new(),
    }
}

fn card() -> CanvasEntity {
    let mut card = entity(Uuid::from_u128(1));
    card.transform =
        CanvasTransform::new(CanvasPoint::default(), CanvasPoint::new(180.0, 120.0), 0.0);
    card.style.fill = Some(block_editor_beui::be_block::canvas::CanvasColor::Rgba {
        red: 60,
        green: 110,
        blue: 90,
        alpha: 255,
    });
    card
}

fn open(canvas: &Canvas, preview: bool, known: &[BlockInfo]) -> BeuiTest<CanvasApp> {
    open_sized(canvas, preview, known, None)
}

fn open_sized(
    canvas: &Canvas,
    preview: bool,
    known: &[BlockInfo],
    size: Option<Vec2>,
) -> BeuiTest<CanvasApp> {
    let block = Uuid::new_v4();
    let host = EditorHost::default();
    host.set_editable(true);
    let editor = Editor::new(host.clone(), block);
    let test = match preview {
        true => BeuiTest::preview(editor),
        false => BeuiTest::new(editor),
    };
    let test = match size {
        Some(size) => test.with_size(size).with_phone_bar(1),
        None => test,
    };
    let mut harness = test.in_viewport();
    for info in known {
        harness.store().add_block(info.clone());
    }
    harness.hold(None, CanvasContent::new(canvas));
    harness.run();
    harness
}

fn editor(entities: &[CanvasEntity]) -> BeuiTest<CanvasApp> {
    open(&Canvas::with_entities(entities.to_vec()), false, &[])
}

fn phone(entities: &[CanvasEntity]) -> BeuiTest<CanvasApp> {
    open_sized(
        &Canvas::with_entities(entities.to_vec()),
        false,
        &[],
        Some(Vec2::new(390.0, 760.0)),
    )
}

fn apply(editor: &mut BeuiTest<CanvasApp>, operation: InfiniteCanvasOperation) {
    let edit = editor
        .content::<CanvasContent>(None)
        .root()
        .edit_for(&operation);
    editor.edit::<CanvasContent>(None, &edit);
}

fn entities(editor: &BeuiTest<CanvasApp>) -> Vec<CanvasEntity> {
    editor.content::<CanvasContent>(None).root().entities()
}

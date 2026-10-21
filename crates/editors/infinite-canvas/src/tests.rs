use std::collections::{BTreeMap, HashSet};

use block_editor_beui::be_block::canvas::Canvas;
use block_editor_beui::be_block::canvas::{
    CanvasArrowhead, CanvasColor, CanvasComponent, CanvasEntity, CanvasFill, CanvasPathPoint,
    CanvasPoint, CanvasShape, CanvasTransform, InfiniteCanvasOperation,
};
use block_editor_beui::be_block::database::DatabaseValue;
use block_editor_beui::be_block::{
    CanvasContent, DisplaySettings, Root, Settings, SettingsContent, SettingsProfile,
    SettingsProfileContent,
};
use block_editor_beui::beui::Vec2;
use block_editor_beui::{BlockInfo, BlockParent, EditorHost, OwnedEditor};
use block_ui_test::BeuiTest;
use uuid::Uuid;

use crate::app::CanvasApp;
use crate::app::state::{attach_component, remove_component, set_component_value};
use crate::geometry::{ResizeHandle, entity_bounds, resize_entities_axis};

mod a_direct_editor_entity_draws_the_frame_it_reserves;
mod a_dock_too_wide_for_the_canvas_scrolls_to_its_last_tools;
mod a_dropped_slide_takes_its_own_size_rather_than_the_cards;
mod a_finger_drags_a_transform_field_in_the_sheet;
mod a_finger_just_outside_a_corner_resizes_the_selection;
mod a_finger_on_the_pen_tool_draws_before_it_moves;
mod a_frame_background_is_picked_in_the_sidebar;
mod a_held_finger_opens_the_context_menu_in_every_tool;
mod a_live_editor_works_in_place_and_a_pan_and_zoom_one_takes_the_frame_when_clicked;
mod a_moved_entity_is_drawn_where_it_was_dropped;
mod a_multiple_selection_outlines_each_selected_entity;
mod a_narrow_canvas_opens_its_inspector_under_the_stage;
mod a_phone_canvas_keeps_its_tools_in_a_dock_and_zoom_pill;
mod a_preview_with_no_camera_fits_its_frame;
mod a_resize_handle_follows_the_drag_rather_than_jumping_to_the_pointer;
mod a_second_finger_calls_off_the_shape_being_drawn;
mod a_short_finger_stroke_keeps_every_point;
mod a_typed_transform_value_is_one_edit;
mod an_arrow_drawn_between_two_shapes_follows_them_when_they_move;
mod an_image_still_loading_is_drawn_as_its_thumbhash;
mod attaching_component_fills_only_missing_selected_entities;
mod clicking_an_entity_selects_it_and_shows_its_handles;
mod dragging_a_transform_field_twice_keeps_the_first_drag;
mod dragging_an_end_of_a_selected_line_moves_only_that_end;
mod dragging_any_transform_field_moves_the_entity;
mod dragging_one_entity_wakes_only_that_entity;
mod dragging_with_the_frame_tool_adds_a_frame;
mod dragging_with_the_rectangle_tool_adds_a_rectangle;
mod drawing_with_a_finger_adds_a_rectangle;
mod every_shape_is_drawn_in_the_clean_look;
mod every_transform_field_previews_what_is_typed;
mod once_a_pen_has_drawn_a_finger_in_the_pen_tool_selects;
mod panning_and_zooming_paints_no_entity_again;
mod pasting_puts_the_objects_under_the_pointer;
mod removing_component_deletes_its_values_from_all_selected_entities;
mod replacing_a_referenced_block_rewrites_the_entity;
mod resizing_an_editor_that_cannot_resize_scales_it;
mod rotating_with_the_handle_is_drawn;
mod selections_are_shared_with_peers_and_theirs_are_drawn;
mod setting_component_value_writes_the_same_value_to_all_selected_entities;
mod tapping_selected_text_on_a_phone_edits_it;
mod text_is_edited_where_it_is_drawn;
mod text_sits_against_the_edges_it_is_aligned_to;
mod the_actions_menu_deletes_the_selection;
mod the_canvas_opens_with_the_select_tool_so_a_finger_box_selects;
mod the_canvas_paints_the_entities_it_holds;
mod the_ellipse_and_diamond_tools_draw_their_shapes;
mod the_eraser_end_of_a_pen_deletes_what_it_touches;
mod the_intrinsic_size_follows_the_first_frame;
mod the_layers_panel_selects_reorders_and_hides;
mod the_palette_and_the_shortcuts_run_the_canvas_actions;
mod the_pen_barrel_button_opens_the_context_menu;
mod the_pen_tool_draws_a_stylus_stroke_as_wide_as_it_is_pressed;
mod the_pen_tool_keeps_every_coalesced_sample_and_draws_the_predicted_tail;
mod the_preview_fills_the_frame_with_its_background;
mod the_preview_shows_the_first_frame;
mod the_transform_fields_edit_the_selected_entity;
mod turning_touch_off_in_the_dock_keeps_fingers_off_the_canvas;
mod typed_text_grows_away_from_the_edges_it_is_aligned_to;
mod typing_a_transform_value_and_pressing_escape_edits_nothing;

fn shaped(id: Uuid, shape: CanvasShape) -> CanvasEntity {
    let mut entity = CanvasEntity::new(
        shape,
        CanvasTransform::new(CanvasPoint::default(), CanvasPoint::new(10.0, 10.0), 0.0),
    );
    entity.id = id;
    entity.style.seed = id.as_u128() as u32;
    entity
}

fn entity(id: Uuid) -> CanvasEntity {
    shaped(id, CanvasShape::Rectangle)
}

fn path(id: Uuid, world: &[CanvasPoint]) -> CanvasEntity {
    let mut path = shaped(id, CanvasShape::Path);
    path.set_world_points(world.iter().copied().map(CanvasPathPoint::new).collect());
    path
}

fn frame(id: u128, center: CanvasPoint, size: CanvasPoint) -> CanvasEntity {
    let mut frame = shaped(Uuid::from_u128(id), CanvasShape::Frame);
    frame.transform = CanvasTransform::new(center, size, 0.0);
    frame.text = format!("Frame {id}");
    frame
}

fn card() -> CanvasEntity {
    let mut card = entity(Uuid::from_u128(1));
    card.transform =
        CanvasTransform::new(CanvasPoint::default(), CanvasPoint::new(180.0, 120.0), 0.0);
    card.style.fill = CanvasFill::Solid;
    card.style.fill_color = CanvasColor::Rgba {
        red: 60,
        green: 110,
        blue: 90,
        alpha: 255,
    };
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
    let editor = OwnedEditor::new(host.clone(), block);
    let test = match preview {
        true => BeuiTest::preview(editor),
        false => BeuiTest::new(editor),
    };
    let test = match size {
        Some(size) => test.with_size(size).on_phone(),
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

fn display_settings_here(editor: &mut BeuiTest<CanvasApp>) -> Uuid {
    let client = editor.host().client_id();
    let (settings, profile, display) = (Uuid::new_v4(), Uuid::new_v4(), Uuid::new_v4());
    let store = editor.store();
    store.add_block(BlockInfo::new(
        settings,
        Settings::CONTENT_TYPE,
        BlockParent::Root,
    ));
    store.add_block(BlockInfo::new(
        profile,
        SettingsProfile::CONTENT_TYPE,
        BlockParent::Block(settings),
    ));
    let mut held = SettingsContent::default();
    held.apply(&Settings::add_profile(client, profile));
    store.hold(Some(settings), held);
    let mut own = SettingsProfileContent::default();
    own.apply(&SettingsProfile::set_kind(
        DisplaySettings::CONTENT_TYPE,
        display,
    ));
    store.hold(Some(profile), own);
    store.hold(
        Some(display),
        block_editor_beui::be_block::DisplaySettingsContent::default(),
    );
    editor.run();
    display
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

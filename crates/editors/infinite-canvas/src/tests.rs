use std::collections::{BTreeMap, HashSet};

use block_editor_beui::be_block::presence::PresenceKind;
use block_editor_beui::be_block::{
    DisplaySettings, Root, Settings, SettingsContent, SettingsProfile, SettingsProfileContent,
};
use block_editor_beui::beui::Vec2;
use block_editor_beui::{BlockInfo, BlockParent, EditorHost, OwnedEditor};
use block_ui_test::BeuiTest;
use database_types::database::DatabaseValue;
use infinite_canvas_types::Canvas;
use infinite_canvas_types::{
    CanvasArrowhead, CanvasColor, CanvasComponent, CanvasEntity, CanvasFill, CanvasLook,
    CanvasPathPoint, CanvasPoint, CanvasProfile, CanvasShape, CanvasStrokeStyle, CanvasTextSize,
    CanvasTransform, InfiniteCanvasOperation,
};
use uuid::Uuid;

use crate::app::CanvasApp;
use crate::app::state::{attach_component, remove_component, set_component_value};
use crate::geometry::{ResizeHandle, entity_bounds, resize_entities_axis};
use block_editor_beui::beui::{PenPhase, PointerButton};
use infinite_canvas_types::CanvasContent;

mod a_direct_editor_entity_draws_the_frame_it_reserves;
mod a_dock_too_wide_for_the_canvas_scrolls_to_its_last_tools;
mod a_drag_in_a_label_being_edited_selects_it_and_leaves_the_shape_in_place;
mod a_drag_in_text_being_edited_selects_it_and_leaves_the_text_in_place;
mod a_drawing_tool_returns_to_select_unless_the_tool_is_locked;
mod a_dropped_slide_takes_its_own_size_rather_than_the_cards;
mod a_finger_drags_a_transform_field_in_the_sheet;
mod a_finger_just_outside_a_corner_resizes_the_selection;
mod a_finger_on_the_pen_tool_draws_before_it_moves;
mod a_frame_background_is_picked_in_the_sidebar;
mod a_growing_stroke_writes_only_its_new_points;
mod a_hand_drawn_shape_is_the_same_for_the_same_seed;
mod a_held_finger_opens_the_context_menu_in_every_tool;
mod a_letter_typed_into_a_field_picks_no_tool;
mod a_line_is_bound_while_it_is_drawn_onto_a_shape;
mod a_live_editor_works_in_place_and_a_pan_and_zoom_one_takes_the_frame_when_clicked;
mod a_long_stroke_being_drawn_shows_no_seams_between_its_pieces;
mod a_moved_entity_is_drawn_where_it_was_dropped;
mod a_multiple_selection_outlines_each_selected_entity;
mod a_narrow_canvas_opens_its_inspector_under_the_stage;
mod a_new_shape_takes_the_last_style_used;
mod a_peer_sees_a_labelled_shape_while_it_is_moved;
mod a_peer_sees_a_move_called_off_put_back;
mod a_peer_sees_a_shape_while_it_is_dragged;
mod a_peer_sees_a_stroke_while_it_is_drawn;
mod a_peer_sees_where_another_is_writing;
mod a_peer_types_into_the_text_being_edited;
mod a_pen_lift_catalogues_only_the_new_stroke;
mod a_pen_move_paints_and_visits_no_entity_again;
mod a_phone_canvas_keeps_its_tools_in_a_dock_and_zoom_pill;
mod a_phone_opens_a_context_submenu_as_a_page_of_its_sheet;
mod a_png_export_draws_formatted_text_as_the_canvas_does;
mod a_preview_with_no_camera_fits_its_frame;
mod a_resize_handle_follows_the_drag_rather_than_jumping_to_the_pointer;
mod a_second_finger_calls_off_the_shape_being_drawn;
mod a_selected_bound_arrow_shows_where_each_end_is_anchored;
mod a_selection_is_copied_and_exported_as_png;
mod a_shape_is_drawn_while_dragged_as_it_lands;
mod a_short_finger_stroke_keeps_every_point;
mod a_stroke_drawn_in_pieces_has_the_outline_of_the_whole_stroke;
mod a_text_area_keeps_its_box_when_entered_and_typed_into;
mod a_text_label_draws_its_lists_and_quotes;
mod a_text_tapped_in_starts_at_the_tap;
mod a_typed_transform_value_is_one_edit;
mod a_view_the_host_sets_is_not_fitted_back;
mod an_arrow_drawn_between_two_shapes_follows_them_when_they_move;
mod an_export_marks_the_list_markers_and_quote_bars_it_cannot_draw;
mod an_image_still_loading_is_drawn_as_its_thumbhash;
mod attaching_component_fills_only_missing_selected_entities;
mod clicking_an_entity_selects_it_and_shows_its_handles;
mod copying_as_svg_writes_every_shape_kind;
mod double_clicking_a_group_enters_it;
mod double_clicking_the_empty_canvas_writes_text_there;
mod dragging_a_transform_field_twice_keeps_the_first_drag;
mod dragging_an_end_of_a_selected_line_moves_only_that_end;
mod dragging_any_transform_field_moves_the_entity;
mod dragging_one_entity_wakes_only_that_entity;
mod dragging_the_eraser_dims_what_it_crosses_and_erases_it_on_release;
mod dragging_the_middle_of_a_line_bends_it_at_a_new_point;
mod dragging_with_the_frame_tool_adds_a_frame;
mod dragging_with_the_rectangle_tool_adds_a_rectangle;
mod drawing_with_a_finger_adds_a_rectangle;
mod enter_on_a_selected_shape_edits_its_label;
mod every_frame_of_a_drag_draws_a_bound_end_on_the_moved_shape;
mod every_shape_is_drawn_in_the_clean_look;
mod every_shape_is_drawn_in_the_hand_drawn_look;
mod every_transform_field_previews_what_is_typed;
mod fit_all_holds_when_the_window_resizes;
mod fit_all_on_an_empty_canvas_returns_to_the_default_view;
mod fit_all_without_frames_zooms_to_everything;
mod fit_all_zooms_to_the_first_frame;
mod frames_hide_the_grid_outside_them_and_keep_a_hairline_outline;
mod freehand_is_drawn_as_an_outline_that_follows_pressure;
mod once_a_pen_has_drawn_a_finger_in_the_pen_tool_selects;
mod one_undo_takes_back_a_whole_gesture;
mod panning_and_zooming_paints_no_entity_again;
mod panning_paints_no_frame_backdrop_again;
mod panning_paints_no_grid_dot_again;
mod panning_paints_no_selection_handle_again;
mod pasting_puts_the_objects_under_the_pointer;
mod pen_strokes_are_smooth_curves_at_any_zoom;
mod removing_component_deletes_its_values_from_all_selected_entities;
mod replacing_a_referenced_block_rewrites_the_entity;
mod resizing_an_editor_that_cannot_resize_scales_it;
mod rotating_with_the_handle_is_drawn;
mod scrubbing_a_number_is_one_undo_step;
mod selections_are_shared_with_peers_and_theirs_are_drawn;
mod setting_component_value_writes_the_same_value_to_all_selected_entities;
mod simulated_pressure_is_the_same_at_any_sample_rate;
mod switching_the_profile_changes_this_viewers_chrome_and_the_default;
mod switching_to_vector_draws_the_next_shape_clean;
mod tabbing_through_the_dock_shows_each_focus_ring_whole;
mod tapping_selected_text_on_a_phone_edits_it;
mod text_is_edited_where_it_is_drawn;
mod text_left_empty_leaves_nothing_behind;
mod text_shrinks_with_a_zoom_out_before_it_is_shaped_again;
mod text_sits_against_the_edges_it_is_aligned_to;
mod text_tapped_in_where_the_keyboard_opens_keeps_the_keyboard;
mod text_written_near_the_bottom_stays_in_view_when_the_keyboard_opens;
mod the_actions_menu_deletes_the_selection;
mod the_canvas_opens_with_the_select_tool_so_a_finger_box_selects;
mod the_canvas_paints_the_entities_it_holds;
mod the_dock_ends_with_ignore_touch_after_the_block_button;
mod the_ellipse_and_diamond_tools_draw_their_shapes;
mod the_eraser_end_of_a_pen_deletes_what_it_touches;
mod the_eraser_finds_shapes_where_they_have_moved_to;
mod the_grid_is_a_setting_of_the_canvas_drawn_behind_it;
mod the_intrinsic_size_follows_the_first_frame;
mod the_lasso_selects_only_what_it_encloses;
mod the_layers_panel_selects_reorders_and_hides;
mod the_letters_and_numbers_pick_every_tool;
mod the_palette_and_the_shortcuts_run_the_canvas_actions;
mod the_pen_barrel_button_opens_the_context_menu;
mod the_pen_tool_draws_a_stylus_stroke_as_wide_as_it_is_pressed;
mod the_pen_tool_keeps_every_coalesced_sample_and_draws_the_predicted_tail;
mod the_preview_fills_the_frame_with_its_background;
mod the_preview_shows_the_first_frame;
mod the_style_popover_fits_a_phone_above_the_dock;
mod the_style_popover_restyles_the_selection;
mod the_text_tool_writes_its_box_while_it_is_dragged;
mod the_tool_bar_docked_along_the_bottom_takes_room_from_the_stage;
mod the_tool_bar_moved_to_the_left_stands_its_tools_in_a_column;
mod the_tool_bar_pops_out_of_the_canvas_into_a_window;
mod the_transform_fields_edit_the_selected_entity;
mod turned_text_is_drawn_turned_as_it_is_exported;
mod turning_touch_off_in_the_dock_keeps_fingers_off_the_canvas;
mod typed_text_grows_away_from_the_edges_it_is_aligned_to;
mod typing_a_transform_value_and_pressing_escape_edits_nothing;
mod with_binding_off_or_ctrl_held_a_line_drawn_onto_a_shape_stays_free;
mod zoomed_text_wraps_the_same_while_edited_and_takes_typing_where_pressed;

fn colored(red: u8, green: u8, blue: u8) -> CanvasColor {
    CanvasColor::Rgba {
        red,
        green,
        blue,
        alpha: 255,
    }
}

fn placed(id: u128, shape: CanvasShape, center: CanvasPoint) -> CanvasEntity {
    let mut entity = shaped(Uuid::from_u128(id), shape);
    entity.transform = CanvasTransform::new(center, CanvasPoint::new(120.0, 80.0), 0.0);
    entity
}

fn every_shape(look: CanvasLook) -> Vec<CanvasEntity> {
    let mut ellipse = placed(1, CanvasShape::Ellipse, CanvasPoint::new(-160.0, -100.0));
    ellipse.style.fill = CanvasFill::Solid;
    ellipse.style.fill_color = colored(250, 200, 80);
    let mut diamond = placed(2, CanvasShape::Diamond, CanvasPoint::new(0.0, -100.0));
    diamond.style.stroke_style = CanvasStrokeStyle::Dashed;
    diamond.style.stroke_color = colored(25, 113, 194);
    let mut labelled = placed(3, CanvasShape::Rectangle, CanvasPoint::new(160.0, -100.0));
    labelled.text = "Label".into();
    labelled.style.text_size = CanvasTextSize::Small;

    let mut polyline = path(
        Uuid::from_u128(4),
        &[
            CanvasPoint::new(-220.0, 20.0),
            CanvasPoint::new(-140.0, 80.0),
            CanvasPoint::new(-60.0, 20.0),
        ],
    );
    polyline.style.start_arrowhead = CanvasArrowhead::Arrow;
    polyline.style.end_arrowhead = CanvasArrowhead::Arrow;
    polyline.style.stroke_style = CanvasStrokeStyle::Dotted;

    let mut curve = path(
        Uuid::from_u128(5),
        &[CanvasPoint::new(0.0, 20.0), CanvasPoint::new(0.0, 100.0)],
    );
    let mut world = curve.world_points();
    world[0].handle_out = Some(CanvasPoint::new(80.0, 20.0));
    world[1].handle_in = Some(CanvasPoint::new(-80.0, 100.0));
    curve.set_world_points(world);
    curve.text = "curve".into();
    curve.style.text_size = CanvasTextSize::Small;

    let mut freehand = path(
        Uuid::from_u128(6),
        &[
            CanvasPoint::new(120.0, 30.0),
            CanvasPoint::new(160.0, 70.0),
            CanvasPoint::new(200.0, 40.0),
        ],
    );
    freehand.shape = CanvasShape::Freehand;
    freehand.pressures = vec![0.2, 0.9, 0.5];

    let mut hatched = placed(7, CanvasShape::Rectangle, CanvasPoint::new(-160.0, 160.0));
    hatched.style.fill = CanvasFill::Hatched;
    hatched.style.fill_color = colored(224, 49, 49);
    hatched.style.corner_radius = 16.0;
    let mut crossed = placed(8, CanvasShape::Ellipse, CanvasPoint::new(0.0, 160.0));
    crossed.style.fill = CanvasFill::Hatched;
    crossed.style.fill_color = colored(47, 158, 68);
    crossed.transform.rotation = 0.3;
    let mut solid = placed(9, CanvasShape::Diamond, CanvasPoint::new(160.0, 160.0));
    solid.style.fill = CanvasFill::Solid;
    solid.style.fill_color = colored(25, 113, 194);
    solid.style.stroke_width = 4.0;

    let mut every = vec![
        ellipse, diamond, labelled, polyline, curve, freehand, hatched, crossed, solid,
    ];
    for entity in &mut every {
        entity.style.look = look;
    }
    every
}

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
    frame.text = format!("Frame {id}").as_str().into();
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
    harness.store().keep_history::<CanvasContent>(None);
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

fn vector(entities: impl IntoIterator<Item = CanvasEntity>) -> Canvas {
    let mut canvas = Canvas::with_entities(entities);
    canvas.profile = CanvasProfile::Vector;
    canvas
}

fn vector_editor(entities: &[CanvasEntity]) -> BeuiTest<CanvasApp> {
    open(&vector(entities.to_vec()), false, &[])
}

fn vector_phone(entities: &[CanvasEntity]) -> BeuiTest<CanvasApp> {
    open_sized(
        &vector(entities.to_vec()),
        false,
        &[],
        Some(Vec2::new(390.0, 760.0)),
    )
}

fn apply(editor: &mut BeuiTest<CanvasApp>, operation: InfiniteCanvasOperation) {
    let edit = Canvas::edit_in(&editor.content::<CanvasContent>(None), &operation);
    editor.edit::<CanvasContent>(None, &edit);
}

fn entities(editor: &BeuiTest<CanvasApp>) -> Vec<CanvasEntity> {
    editor.content::<CanvasContent>(None).root().entities()
}

fn forward_presence(
    from: &mut BeuiTest<CanvasApp>,
    to: &mut BeuiTest<CanvasApp>,
    held: &mut BTreeMap<Uuid, Vec<u8>>,
) {
    for shown in from.take_shown_presence() {
        match shown.value {
            Some(value) => held.insert(shown.kind, value),
            None => held.remove(&shown.kind),
        };
    }
    to.set_peers(
        None,
        held.iter()
            .map(|(kind, value)| block_editor_beui::PeerPresence {
                client: 7,
                kind: *kind,
                value: value.clone(),
            })
            .collect(),
    );
    to.run();
}

fn undo(editor: &mut BeuiTest<CanvasApp>) {
    let block = editor.block_id().expect("the canvas has a block");
    editor.host().undo(block);
    editor.run();
}

fn sync(from: &mut BeuiTest<CanvasApp>, to: &mut BeuiTest<CanvasApp>) {
    for operation in from.store().take_written(None) {
        to.store().apply_elsewhere(None, operation);
    }
    to.run();
}

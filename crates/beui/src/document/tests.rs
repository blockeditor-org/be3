use super::*;

mod a_cancelled_touch_outside_a_dialog_leaves_it_open;
mod a_canvas_item_moved_into_view_is_laid_out_where_it_arrives;
mod a_canvas_lays_out_only_the_items_the_view_can_see;
mod a_canvas_places_its_items_at_the_view_it_is_given;
mod a_canvas_without_a_view_places_its_items_from_its_own_corner;
mod a_capturing_button_takes_the_press_from_the_row_it_sits_in;
mod a_closure_child_receives_the_handle_its_slot_hands_over;
mod a_color_input_reports_the_hex_it_was_typed;
mod a_component_function_returns_its_base_node;
mod a_component_that_builds_no_node_owns_its_scope_through_the_value;
mod a_component_wrapping_a_node_less_component_keeps_its_scope;
mod a_context_menu_item_follows_the_signals_its_tag_was_written_with;
mod a_crowded_dock_tab_bar_scrolls_rather_than_spilling;
mod a_deadline_repaint_only_damages_the_element_that_asked_for_it;
mod a_dialog_opens_in_the_middle_and_escape_dismisses_it;
mod a_disabled_button_prop_tracks_a_signal_and_blocks_clicks_while_true;
mod a_disabled_checkbox_ignores_clicks_and_keeps_its_state;
mod a_disabled_select_does_not_open_when_its_trigger_is_clicked;
mod a_disabled_text_input_ignores_typing_and_reads_as_dimmed;
mod a_dock_tab_in_a_window_opens_its_menu_over_the_window;
mod a_docked_pane_lays_its_content_inside_its_border;
mod a_drag_preview_follows_the_pointer_until_the_drop;
mod a_drawing_paints_what_its_callback_puts_in_the_rectangle_it_is_given;
mod a_drawing_repaints_on_its_deadline_without_repeating_layout;
mod a_dynamic_child_can_fill_its_available_height;
mod a_floating_child_pins_itself_over_the_scroll_it_names;
mod a_focused_text_input_asks_for_the_keyboard;
mod a_for_each_gives_a_scroll_items_of_its_own;
mod a_for_each_keeps_its_rows_between_the_children_beside_it;
mod a_for_each_places_the_items_of_a_canvas;
mod a_for_each_row_picks_and_changes_its_own_size;
mod a_fragment_sits_beside_the_children_written_around_it;
mod a_frame_with_a_max_width_stops_growing_at_it_but_still_shrinks;
mod a_fullscreen_overlay_fills_the_window_and_escape_dismisses_it;
mod a_hidden_show_gives_its_share_of_the_space_to_its_visible_siblings;
mod a_horizontal_scroll_lays_its_items_out_in_a_row;
mod a_keyed_view_rebuilds_only_when_its_key_changes;
mod a_list_sizes_plain_nodes_handed_to_it_intrinsically;
mod a_lone_child_fills_a_children_prop_as_a_run_of_one;
mod a_menu_row_with_a_submenu_shows_an_arrow_the_leaf_rows_do_not;
mod a_multi_root_view_fills_a_children_prop_in_order;
mod a_nested_container_reports_its_own_width_not_the_windows;
mod a_number_input_reports_what_was_typed_within_its_range;
mod a_pan_zoom_follows_the_view_its_caller_sets;
mod a_password_text_area_masks_its_text_and_keeps_it_off_the_clipboard;
mod a_picture_given_a_source_paints_only_that_part_of_the_image;
mod a_picture_paints_the_image_it_is_given;
mod a_picture_scaled_down_never_grows_past_its_own_pixels;
mod a_pointer_lock_lets_go_when_the_window_loses_focus;
mod a_pointer_lock_reports_motion_only_while_it_holds_the_pointer;
mod a_portal_shows_a_subtree_it_does_not_own;
mod a_reactive_sizing_attribute_moves_a_child_between_fixed_and_percent;
mod a_reactive_test_id_follows_its_signal;
mod a_reactive_tree_can_nest_builder_calls_without_threading_the_document;
mod a_row_added_to_a_for_each_keeps_the_sizes_the_rows_beside_it_chose;
mod a_scroll_in_a_dialog_follows_the_wheel;
mod a_scroll_inside_a_scroll_lays_out_the_rows_it_holds;
mod a_scroll_mixes_plain_children_with_a_nested_virtual_list;
mod a_scrollbar_sizes_its_thumb_from_the_scroll_beside_it;
mod a_select_following_its_prop_does_not_report_a_change;
mod a_selected_radio_option_marks_its_ring_with_the_accent_colour;
mod a_selection_handle_takes_a_tap_before_the_button_it_covers;
mod a_shortcut_can_leave_keys_to_the_text_input_that_has_the_focus;
mod a_show_adds_and_removes_a_menu_item_among_the_items_beside_it;
mod a_show_adds_and_removes_a_tab_among_the_tabs_beside_it;
mod a_signal_write_from_a_click_handler_updates_its_bound_text_in_the_same_frame;
mod a_simulated_mouse_click_lands_where_the_trackpad_moved_its_cursor;
mod a_skipped_element_keeps_the_repaint_deadline_it_asked_for;
mod a_slider_reports_and_steps_within_the_range_it_was_given;
mod a_spinner_hidden_by_a_show_stops_asking_for_frames;
mod a_stack_becomes_a_column_when_its_container_gets_narrow;
mod a_stack_built_inside_a_show_still_measures_the_container_above_it;
mod a_stroke_paints_a_line_between_the_ends_it_was_given;
mod a_styled_scroll_puts_its_scrollbar_beside_the_content;
mod a_tab_clicked_within_one_frame_does_not_start_a_drag;
mod a_tab_split_out_of_a_window_keeps_its_panel_on_screen;
mod a_tag_can_take_a_node_ref_and_a_test_id_slot_at_once;
mod a_text_area_shows_its_placeholder_until_something_is_typed;
mod a_text_input_in_a_tall_slot_keeps_its_text_inside_its_field;
mod a_theme_provider_restyles_its_subtree_when_its_theme_changes;
mod a_timer_asks_for_frames_until_its_work_settles;
mod a_tooltip_appears_after_a_dwell_and_leaves_the_control_clickable;
mod a_touch_fling_that_ends_without_moving_keeps_its_momentum;
mod a_tree_row_decides_which_part_of_it_is_clickable;
mod a_two_finger_drag_on_the_simulated_trackpad_scrolls_smoothly;
mod a_value_written_between_tags_takes_the_sizing_after_it;
mod a_viewport_fills_the_space_it_is_given;
mod a_virtual_list_in_a_stacked_stack_only_builds_the_items_in_view;
mod a_virtual_list_reaches_the_end_when_rows_outgrow_their_estimate;
mod a_virtual_list_scrolled_out_of_view_releases_its_rows;
mod a_virtual_scroll_only_builds_the_items_in_view;
mod a_virtual_scroll_row_can_build_reactive_content_during_dispatch;
mod a_window_with_tabs_in_a_sidebar_has_no_title_bar;
mod a_wrapping_caption_grows_taller_than_the_single_line_it_would_be;
mod a_wrapping_row_flows_its_children_onto_more_lines;
mod accessibility_exposes_and_operates_a_button;
mod accessibility_reports_and_steps_a_slider;
mod accessibility_updates_leave_the_tree_a_fresh_build_would_make;
mod alt_arrows_walk_the_simulated_screen_reader_through_the_document;
mod alt_dragging_a_tab_floats_it_in_a_window_over_the_pane_it_left;
mod an_aspect_ratio_frame_centres_the_largest_box_that_fits;
mod an_embed_punches_a_hole_in_the_surface_it_sits_on;
mod an_embed_reports_a_rect_on_the_pixel_grid;
mod an_embed_reports_the_rect_and_the_clip_it_was_laid_out_in;
mod an_empty_field_shows_its_placeholder_until_something_is_typed;
mod an_empty_view_builds_a_children_prop_with_nothing_in_it;
mod an_icon_is_as_tall_as_the_text_it_sits_with;
mod an_idle_frame_describes_no_accessibility_nodes_and_one_changed_row_describes_few;
mod an_offset_leaves_the_wheel_to_the_scroll_around_it;
mod an_optional_child_slot_takes_no_children_or_exactly_one;
mod an_unstyled_scroll_keeps_the_whole_width_for_its_content;
mod appending_to_a_virtual_list_keeps_the_rows_it_built;
mod arrow_down_on_a_closed_select_trigger_opens_it_and_highlights_the_first_option;
mod arrow_keys_in_a_select_search_box_move_the_highlighted_option_without_editing_the_search_text;
mod arrow_keys_move_a_visible_highlight_through_an_open_context_menu;
mod arrow_keys_pan_a_focused_pan_zoom;
mod arrow_keys_step_a_curved_slider_evenly_along_its_track;
mod arrow_keys_step_the_focused_slider;
mod arrow_keys_walk_the_rows_of_the_inspector_tree;
mod backspace_deletes_the_character_before_the_caret;
mod children_written_between_show_tags_are_not_built_until_it_is_shown;
mod choosing_the_e_ink_theme_in_the_inspector_restyles_the_document;
mod clicking_a_checkbox_toggles_it;
mod clicking_a_link_reports_it_and_reads_as_a_link;
mod clicking_a_row_collapses_its_children;
mod clicking_a_row_selects_the_node_it_lists;
mod clicking_a_switch_moves_its_knob_and_survives_a_tab_round_trip;
mod clicking_a_tab_selects_the_panel_it_names;
mod clicking_a_tree_marker_expands_the_row_without_opening_it;
mod clicking_a_window_title_bar_takes_the_focus_out_of_a_group;
mod clicking_an_accordion_header_hides_its_content;
mod clicking_outside_an_open_select_popup_closes_it_without_clicking_through;
mod clicking_the_inspector_close_button_closes_the_panel;
mod clicking_the_middle_of_a_placeholder_puts_the_caret_at_the_start;
mod clicking_the_outer_tab_bar_takes_the_focus_out_of_a_group;
mod clicking_the_padding_around_a_button_label_activates_it;
mod clicking_the_scrollbar_track_pages_the_scroll_towards_the_click;
mod clicking_the_start_of_a_text_input_puts_the_caret_before_the_text;
mod compacting_virtual_rows_clamps_the_scroll_anchor_at_the_end;
mod ctrl_a_selects_everything_so_typing_replaces_the_value;
mod ctrl_f_opens_the_find_bar_over_a_text_area;
mod ctrl_scrolling_a_pan_zoom_zooms_around_the_pointer;
mod ctrl_shift_f_moves_focus_between_the_inspector_and_the_document;
mod ctrl_shift_i_opens_and_closes_the_inspector;
mod ctrl_tab_walks_the_tabs_of_the_pane_the_focus_is_in;
mod ctrl_z_undoes_what_was_typed_into_a_text_input;
mod dock_tabs_are_the_same_height_whether_or_not_they_close;
mod dock_tabs_moved_into_a_sidebar_stack_beside_the_panel;
mod double_clicking_a_word_selects_it_so_typing_replaces_it;
mod double_tapping_with_the_screen_reader_activates_what_it_is_reading;
mod dragging_a_curved_slider_reads_its_midpoint_at_the_centre;
mod dragging_a_number_input_sideways_changes_its_value;
mod dragging_a_pan_zoom_with_the_middle_button_pans_it;
mod dragging_a_panes_grip_moves_every_tab_of_the_pane;
mod dragging_a_slider_moves_its_value;
mod dragging_a_tab_onto_a_window_bar_moves_it_into_the_window;
mod dragging_a_tab_onto_the_edge_of_a_pane_splits_it;
mod dragging_a_tab_over_a_window_bar_marks_where_it_lands;
mod dragging_a_tab_past_the_one_beside_it_reorders_the_tab_bar;
mod dragging_a_tab_within_a_window_bar_reorders_it;
mod dragging_a_window_by_its_bar_moves_it;
mod dragging_again_during_overscroll_continues_from_the_band;
mod dragging_onto_a_drop_target_hands_it_the_payload;
mod dragging_the_bar_between_two_panes_moves_the_boundary;
mod dragging_the_edge_of_a_windows_sidebar_resizes_it_without_moving_the_window;
mod dragging_the_end_handle_of_a_double_tapped_word_extends_the_selection;
mod dragging_the_inspector_edge_resizes_the_panel;
mod dragging_the_scrollbar_thumb_scrolls_the_content_beside_it;
mod dropping_a_dock_tab_onto_the_middle_of_another_groups_them;
mod editing_one_row_of_a_keyed_list_leaves_every_node_in_place;
mod enabling_touch_emulation_in_the_inspector_does_not_paint_a_pointer;
mod enter_activates_a_list_row_and_space_selects_it;
mod enter_activates_the_focused_button;
mod enter_confirms_the_highlighted_select_option_and_closes_the_popup;
mod enter_in_a_single_line_text_area_submits_rather_than_breaking_the_line;
mod enter_on_an_inspector_row_selects_it_and_toggles_its_children;
mod enter_toggles_the_focused_checkbox;
mod escape_closes_an_open_select_popup_and_returns_focus_to_the_trigger;
mod evicting_a_virtual_scroll_row_disposes_its_effects;
mod finding_a_node_by_its_test_id;
mod flashing_changed_elements_outlines_the_node_that_changed;
mod flashing_repaints_outlines_only_the_region_whose_shapes_changed;
mod flashing_repaints_paints_no_fill;
mod flicking_across_the_screen_reader_reads_the_next_item;
mod flipping_a_switch_can_replace_the_items_of_a_scroll;
mod for_each_reuses_nodes_for_keys_that_persist_across_an_update;
mod grabbing_the_bar_between_two_panes_off_centre_moves_it_only_as_far_as_the_pointer;
mod holding_the_caret_handle_below_a_short_text_area_keeps_scrolling;
mod holding_the_caret_handle_past_the_edge_of_a_narrow_input_keeps_scrolling;
mod holding_the_simulated_left_button_drags_while_another_finger_moves_the_cursor;
mod hovering_a_context_menu_item_moves_keyboard_focus_to_it;
mod hovering_a_link_underlines_it_without_moving_anything;
mod hovering_a_menu_item_with_children_opens_its_submenu_without_a_click;
mod hovering_a_row_highlights_the_node_it_lists;
mod hovering_a_select_option_moves_the_keyboard_highlight;
mod hovering_a_text_input_shows_the_text_cursor;
mod inserting_above_a_virtual_list_view_keeps_the_rows_in_place;
mod inserting_into_a_virtual_list_view_builds_only_the_new_row;
mod jumping_up_a_virtual_scroll_only_builds_the_items_in_view;
mod keys_without_alt_reach_the_control_the_screen_reader_focused;
mod moving_a_dock_tab_to_another_pane_keeps_its_panel;
mod opening_a_menu_damages_only_where_it_appears;
mod opening_a_select_focuses_its_search_box_and_highlights_the_selected_option;
mod painting_never_has_to_move_a_rect_onto_the_pixel_grid;
mod painting_skips_the_elements_outside_the_damaged_region;
mod percent_children_land_on_whole_device_pixels;
mod percent_children_of_an_unbounded_list_use_their_intrinsic_length;
mod percent_sized_children_still_size_an_intrinsic_lists_height;
mod performance_measurements_report_work_and_cache_hits;
mod picking_a_node_leaves_the_document_alone;
mod picking_a_node_reveals_it_in_the_tree;
mod picking_a_node_scrolls_the_inspector_tree_to_its_row;
mod pinching_a_pan_zoom_with_two_fingers_zooms_and_pans_it;
mod pinching_a_pan_zoom_zooms_around_the_pointer;
mod plus_and_minus_zoom_a_focused_pan_zoom_and_zero_resets_the_scale;
mod pressing_enter_past_the_bottom_of_a_text_area_scrolls_the_caret_into_view;
mod quadruple_clicking_selects_everything_so_typing_replaces_the_value;
mod removing_a_keyed_node_drops_the_test_ids_it_registered;
mod removing_a_node_forgets_which_layout_pass_placed_it;
mod removing_a_node_runs_the_cleanups_its_components_registered;
mod removing_a_node_stops_the_effects_that_were_built_for_it;
mod removing_a_node_with_an_open_tooltip_leaves_nothing_to_paint;
mod removing_from_a_virtual_list_view_disposes_only_that_row;
mod required_props_can_be_written_in_any_order_and_as_children;
mod resizing_a_virtual_scroll_reuses_visible_items;
mod resizing_an_element_damages_where_it_was_and_where_it_moved_to;
mod resizing_rows_preserves_the_scroll_anchor;
mod right_arrow_opens_a_submenu_and_left_arrow_closes_it_and_refocuses_the_parent_item;
mod right_click_opens_a_context_menu_at_the_cursor_position;
mod right_clicking_a_dock_tab_pops_it_out_into_a_window;
mod scrolling_a_nested_scroll_leaves_the_one_around_it_alone;
mod scrolling_a_pan_zoom_leaves_the_scroll_around_it_alone;
mod scrolling_a_pan_zoom_pans_it;
mod scrolling_a_virtual_scroll_replaces_the_items_in_view;
mod scrolling_a_virtual_scroll_reuses_overlapping_items;
mod scrolling_back_up_a_virtual_list_keeps_its_rows_adjacent;
mod scrolling_damages_nothing_outside_the_scroll;
mod scrolling_into_a_nested_scroll_keeps_moving_the_one_around_it;
mod scrolling_resends_the_rows_that_moved_but_not_what_moved_with_them;
mod selecting_a_leaf_item_in_a_nested_context_menu_closes_the_whole_menu_stack;
mod setting_the_value_of_a_text_input_reports_the_change;
mod shift_arrow_selects_the_character_that_typing_then_replaces;
mod shift_scrolling_a_horizontal_scroll_moves_it_sideways;
mod shift_scrolling_a_pan_zoom_pans_it_sideways;
mod shift_tab_moves_focus_to_the_previous_button;
mod show_lazily_builds_and_toggles_its_child_when_the_condition_changes;
mod simulating_a_device_pixel_ratio_in_the_inspector_changes_the_pixels_per_point;
mod sizing_attributes_on_the_roots_of_a_multi_root_view_are_honoured;
mod splitting_a_dock_tab_with_the_next_shows_both_side_by_side;
mod swiping_the_simulated_middle_button_scrolls_in_ticks;
mod switching_a_show_damages_both_panels;
mod switching_dock_tabs_keeps_the_panel_it_hides;
mod tab_focus_stays_in_the_active_document_when_the_inspector_is_open;
mod tab_is_trapped_inside_an_open_context_menu;
mod tab_moves_focus_from_one_text_input_to_the_next;
mod tab_moves_focus_to_the_next_button;
mod tabs_collapse_into_a_select_when_their_container_is_narrow;
mod tapping_a_checkbox_with_touch_toggles_it;
mod tapping_inside_a_selection_in_a_select_search_box_opens_its_menu;
mod tapping_inside_a_touch_selection_opens_a_menu_that_copies_it;
mod tapping_the_caret_handle_of_a_text_area_opens_a_menu_that_pastes;
mod tapping_the_caret_handle_opens_a_menu_that_asks_the_host_to_paste;
mod the_caret_of_a_focused_text_area_blinks_on_a_deadline;
mod the_caret_of_a_text_input_paints_two_points_wide;
mod the_demo_body_scrolls_rather_than_spilling_off_a_small_window;
mod the_demo_catalog_survives_switching_tabs;
mod the_dock_demo_leaves_a_tab_saying_nothing_is_open;
mod the_dock_demo_opens_a_paper_from_the_files_it_lists;
mod the_focus_ring_of_a_select_hugs_its_trigger_not_the_row_beside_it;
mod the_frame_output_reports_the_region_whose_shapes_changed;
mod the_innermost_drop_target_that_accepts_the_payload_takes_the_drop;
mod the_inspector_follows_nodes_added_to_the_document;
mod the_inspector_keeps_its_native_size_while_a_pixel_ratio_is_simulated;
mod the_inspector_keeps_the_rows_of_nodes_that_survive_an_update;
mod the_inspector_lists_the_document_tree;
mod the_inspector_shows_document_performance;
mod the_inspector_shows_the_accesskit_tree;
mod the_inspector_shows_the_base_nodes_of_a_styled_component;
mod the_left_and_right_arrows_collapse_and_expand_an_inspector_row;
mod the_right_arrow_scrolls_a_horizontal_scroll_the_focus_is_in;
mod the_screen_reader_buttons_walk_the_document_and_activate_what_they_reach;
mod the_screen_reader_follows_focus_that_tab_moves;
mod the_screen_reader_keeps_clicks_away_from_the_document;
mod the_screen_reader_readout_sits_at_the_bottom_above_the_filters;
mod the_scroll_position_is_reported_to_its_listener;
mod the_scrollbar_thumb_brightens_under_the_pointer_and_while_it_is_dragged;
mod the_simulate_tab_filters_the_document_without_the_screen_reader;
mod the_simulated_input_bars_take_their_room_out_of_the_document;
mod the_simulated_keyboard_types_into_the_focused_input;
mod touch_dragging_a_horizontal_scroll_moves_it_sideways;
mod touch_dragging_a_scroll_moves_it_without_activating_a_row;
mod touch_dragging_across_a_text_input_does_not_select_its_text;
mod touch_overscroll_bands_without_hovering_a_row;
mod triple_clicking_selects_the_line_so_typing_replaces_the_value;
mod turning_on_the_screen_reader_reads_what_it_is_on;
mod turning_the_accessibility_tree_off_in_the_inspector_stops_building_it;
mod typing_in_a_select_search_box_filters_options_case_insensitively;
mod typing_in_the_inspector_tree_jumps_to_a_matching_row;
mod typing_into_a_focused_text_area_inserts_the_text;
mod typing_into_a_focused_text_input_inserts_the_text;
mod typing_into_an_empty_field_does_not_pick_up_its_placeholder;
mod typing_past_the_end_of_a_narrow_single_line_text_area_keeps_the_caret_in_view;
mod typing_past_the_end_of_a_narrow_text_input_scrolls_the_caret_into_view;
mod up_and_down_in_a_single_line_text_area_move_to_its_ends;
mod view_attributes_can_be_written_without_braces;
mod view_attributes_can_pun_a_bare_name_as_its_own_value;
mod view_children_can_pick_fixed_and_percent_sizing;
mod zooming_a_pan_zoom_stops_at_its_scale_limits;

use std::cell::{Cell, RefCell};
use std::rc::Rc;

use crate::color::Color32;
use crate::context::Context;
use crate::geometry::{Pos2, Vec2, pos2};
use crate::input::{Event, Key, Modifiers, PointerButton, RawInput};
use crate::input::{TouchId, TouchPhase};

use crate::base::list::{Direction, ItemSize};
use crate::inspector::Inspector;
use crate::reactive::{
    Canvas, CanvasItem, ClickCallback, ForEach, Frame, Func, List, NodeRef, Offset, Spacer, Text,
    VirtualList, build, create_signal, with_document,
};
use crate::styled;
use crate::unstyled;
use beui_macros::{component, view};

const VIEWPORT: Vec2 = Vec2::new(400.0, 300.0);
const WIDE_VIEWPORT: Vec2 = Vec2::new(1000.0, 600.0);
const TALL_VIEWPORT: Vec2 = Vec2::new(1000.0, 1400.0);
const VIRTUAL_ITEM_COUNT: usize = 10_000;
const VIRTUAL_ITEM_HEIGHT: f32 = 20.0;

fn touch_event(finger: u64, phase: TouchPhase, pos: Pos2) -> Event {
    Event::Touch {
        id: TouchId { device: 1, finger },
        phase,
        pos,
        force: None,
    }
}

pub(crate) struct Harness {
    context: Context,
    document: Document,
    viewport: Vec2,
}

impl Harness {
    pub(crate) fn new(document: Document) -> Self {
        Self {
            context: Context::new(),
            document,
            viewport: VIEWPORT,
        }
    }

    pub(crate) fn sized(document: Document, viewport: Vec2) -> Self {
        Self {
            viewport,
            ..Self::new(document)
        }
    }

    pub(crate) fn viewport_mut(&mut self) -> &mut Vec2 {
        &mut self.viewport
    }

    pub(crate) fn frame(&mut self, events: Vec<Event>) -> crate::FrameOutput {
        let Self {
            context,
            document,
            viewport,
        } = self;
        let input = RawInput { events };
        context.run(input, |context| {
            document.show(context, Rect::from_min_size(Pos2::ZERO, *viewport));
        })
    }

    pub(crate) fn click(&mut self, pos: Pos2) {
        self.frame(vec![Event::PointerMoved(pos)]);
        self.frame(vec![Event::PointerButton {
            pos,
            button: PointerButton::Primary,
            pressed: true,
            modifiers: Modifiers::NONE,
        }]);
        self.frame(vec![Event::PointerButton {
            pos,
            button: PointerButton::Primary,
            pressed: false,
            modifiers: Modifiers::NONE,
        }]);
    }

    pub(crate) fn drag(&mut self, from: Pos2, to: Pos2) {
        self.frame(vec![Event::PointerMoved(from)]);
        self.frame(vec![Event::PointerButton {
            pos: from,
            button: PointerButton::Primary,
            pressed: true,
            modifiers: Modifiers::NONE,
        }]);
        self.frame(vec![Event::PointerMoved(to)]);
        self.frame(vec![Event::PointerButton {
            pos: to,
            button: PointerButton::Primary,
            pressed: false,
            modifiers: Modifiers::NONE,
        }]);
    }

    pub(crate) fn scroll(&mut self, pos: Pos2, delta: Vec2, modifiers: Modifiers) {
        self.frame(vec![
            Event::PointerMoved(pos),
            Event::Modifiers(modifiers),
            Event::Scroll(delta),
        ]);
    }

    pub(crate) fn pinch(&mut self, pos: Pos2, factor: f32) {
        self.frame(vec![Event::PointerMoved(pos), Event::Zoom(factor)]);
    }

    pub(crate) fn middle_drag(&mut self, from: Pos2, to: Pos2) {
        self.frame(vec![Event::PointerMoved(from)]);
        self.frame(vec![Event::PointerButton {
            pos: from,
            button: PointerButton::Middle,
            pressed: true,
            modifiers: Modifiers::NONE,
        }]);
        self.frame(vec![Event::PointerMoved(to)]);
        self.frame(vec![Event::PointerButton {
            pos: to,
            button: PointerButton::Middle,
            pressed: false,
            modifiers: Modifiers::NONE,
        }]);
    }

    pub(crate) fn move_fingers(&mut self, first: Pos2, second: Pos2) {
        self.frame(vec![
            touch_event(1, TouchPhase::Move, first),
            touch_event(2, TouchPhase::Move, second),
        ]);
    }

    pub(crate) fn touch(&mut self, phase: TouchPhase, pos: Pos2) {
        self.finger(1, phase, pos);
    }

    pub(crate) fn finger(&mut self, finger: u64, phase: TouchPhase, pos: Pos2) {
        self.frame(vec![touch_event(finger, phase, pos)]);
    }

    pub(crate) fn key(&mut self, key: Key, modifiers: Modifiers) {
        self.frame(vec![key_event(key, true, modifiers)]);
        self.frame(vec![key_event(key, false, modifiers)]);
    }

    pub(crate) fn type_text(&mut self, text: &str) {
        for letter in text.chars() {
            self.frame(vec![Event::Text(letter.to_string())]);
        }
    }

    pub(crate) fn toggle_inspector(&mut self) {
        self.chord(Key::I);
    }

    pub(crate) fn toggle_picking(&mut self) {
        self.chord(Key::C);
    }

    pub(crate) fn toggle_inspector_focus(&mut self) {
        self.chord(Key::F);
    }

    fn chord(&mut self, key: Key) {
        self.key(
            key,
            Modifiers {
                ctrl: true,
                shift: true,
                ..Modifiers::NONE
            },
        );
    }

    pub(crate) fn accessible(&self) -> Vec<accesskit::Node> {
        self.document
            .accessibility_view()
            .map(|view| view.nodes().cloned().collect())
            .unwrap_or_default()
    }

    pub(crate) fn document(&self) -> &Document {
        &self.document
    }

    pub(crate) fn document_mut(&mut self) -> &mut Document {
        &mut self.document
    }

    pub(crate) fn rect(&self, id: NodeId) -> Rect {
        self.document
            .node_rect(id)
            .expect("the node was not laid out")
    }

    pub(crate) fn find(&self, test_id: &str) -> NodeId {
        self.document
            .find_test_id(test_id)
            .unwrap_or_else(|| panic!("no node with test id {test_id:?}"))
    }

    pub(crate) fn center(&self, id: NodeId) -> Pos2 {
        self.rect(id).center()
    }

    pub(crate) fn inspector(&self) -> &Inspector {
        self.document
            .inspector
            .as_ref()
            .expect("the inspector is closed")
    }

    pub(crate) fn tree(&self) -> Vec<String> {
        self.inspector()
            .entries
            .iter()
            .map(|entry| format!("{}{}", "  ".repeat(entry.depth), entry.kind))
            .collect()
    }

    pub(crate) fn row_center(&self, index: usize) -> Pos2 {
        self.node_center(self.inspector().row_node(index))
    }

    pub(crate) fn focused_row_index(&self) -> Option<usize> {
        let focused = self.inspector().focused_row()?;
        self.inspector()
            .entries
            .iter()
            .position(|entry| entry.key == focused)
    }

    pub(crate) fn touch_toggle_center(&self) -> Pos2 {
        self.inspector_center("inspector.simulation.touch_emulation")
    }

    pub(crate) fn mouse_toggle_center(&self) -> Pos2 {
        self.inspector_center("inspector.simulation.mouse_simulation")
    }

    pub(crate) fn close_button_center(&self) -> Pos2 {
        self.inspector_center("inspector.close")
    }

    pub(crate) fn bar_option_center(&self, index: usize) -> Pos2 {
        self.bar_option_rect(index).center()
    }

    pub(crate) fn bar_option_rect(&self, index: usize) -> Rect {
        self.inspector()
            .bar_option_rect(index)
            .expect("the inspector has no tab bar")
            .scaled(crate::inspector::scale(&self.context))
    }

    pub(crate) fn inspector_panel_rect(&self) -> Rect {
        self.inspector()
            .panel_rect()
            .expect("the inspector panel was not laid out")
            .scaled(crate::inspector::scale(&self.context))
    }

    pub(crate) fn mouse_simulation(&self) -> bool {
        self.context.mouse_simulation()
    }

    pub(crate) fn simulated_cursor(&self) -> Pos2 {
        self.context.simulated_cursor()
    }

    pub(crate) fn simulated_button(&self, index: usize) -> Pos2 {
        self.context.simulated_button(index)
    }

    pub(crate) fn simulated_trackpad(&self) -> Pos2 {
        self.context.simulated_trackpad()
    }

    pub(crate) fn simulated_key(&self, label: &str) -> Pos2 {
        self.context
            .simulated_key(label)
            .unwrap_or_else(|| panic!("the simulated keyboard has no {label:?} key"))
    }

    pub(crate) fn enable_mouse_simulation(&mut self) {
        self.toggle_inspector();
        let tab = self.simulation_tab_center();
        self.click(tab);
        self.frame(Vec::new());
        let toggle = self.mouse_toggle_center();
        self.click(toggle);
        self.key(Key::Escape, Modifiers::NONE);
        self.frame(Vec::new());
    }

    pub(crate) fn point_at(&mut self, target: Pos2) {
        let origin = self.simulated_trackpad();
        let jog = Vec2::new(0.0, -40.0);
        let delta = target - self.simulated_cursor();
        self.finger(9, TouchPhase::Start, origin);
        self.finger(9, TouchPhase::Move, origin + jog);
        self.finger(9, TouchPhase::Move, origin + delta);
        self.finger(9, TouchPhase::End, origin + delta);
    }

    pub(crate) fn tap_trackpad(&mut self) {
        let at = self.simulated_trackpad();
        self.finger(1, TouchPhase::Start, at);
        self.finger(1, TouchPhase::End, at);
        self.frame(Vec::new());
    }

    pub(crate) fn enable_screen_reader(&mut self) {
        self.toggle_inspector();
        let tab = self.simulation_tab_center();
        self.click(tab);
        self.frame(Vec::new());
        let toggle = self.inspector_center("inspector.screen_reader.enabled");
        self.click(toggle);
        self.key(Key::Escape, Modifiers::NONE);
        self.frame(Vec::new());
    }

    pub(crate) fn disable_accessibility(&mut self) {
        self.toggle_inspector();
        let tab = self.simulation_tab_center();
        self.click(tab);
        self.frame(Vec::new());
        let toggle = self.inspector_center("inspector.accessibility.enabled");
        self.click(toggle);
        self.toggle_inspector();
        self.frame(Vec::new());
    }

    pub(crate) fn screen_reader_control_center(&self, control: &str) -> Pos2 {
        self.inspector_center(&format!("inspector.screen_reader.{control}"))
    }

    pub(crate) fn spoken(&self) -> Option<String> {
        self.inspector().reader().spoken()
    }

    pub(crate) fn readout(&self) -> Rect {
        self.inspector().reader().bar()
    }

    pub(crate) fn document_bottom(&self) -> f32 {
        self.document
            .root()
            .and_then(|root| self.document.node_rect(root))
            .expect("the document laid out no root")
            .bottom()
    }

    pub(crate) fn reading(&self) -> Option<String> {
        self.inspector().reader().reading()
    }

    pub(crate) fn reader_item_center(&self, index: usize) -> Pos2 {
        self.inspector().reader().item_center(index)
    }

    pub(crate) fn reader_items(&self) -> Vec<String> {
        self.inspector().reader().items()
    }

    pub(crate) fn change_flash_toggle_center(&self) -> Pos2 {
        self.inspector_center("inspector.performance.flash_changes")
    }

    pub(crate) fn damage_flash_toggle_center(&self) -> Pos2 {
        self.inspector_center("inspector.performance.flash_damage")
    }

    pub(crate) fn accesskit_tab_center(&self) -> Pos2 {
        self.node_center(self.tab_node(1))
    }

    pub(crate) fn performance_tab_center(&self) -> Pos2 {
        self.node_center(self.tab_node(2))
    }

    pub(crate) fn simulation_tab_center(&self) -> Pos2 {
        self.node_center(self.simulation_tab_node())
    }

    pub(crate) fn simulation_tab_node(&self) -> NodeId {
        self.tab_node(3)
    }

    pub(crate) fn pixel_ratio_option_center(&self, index: usize) -> Pos2 {
        let option = self
            .inspector()
            .option_node("inspector.simulation.pixel_ratio", index);
        self.node_center(option)
    }

    pub(crate) fn color_vision_option_center(&self, index: usize) -> Pos2 {
        let option = self
            .inspector()
            .option_node("inspector.simulation.color_vision", index);
        self.node_center(option)
    }

    pub(crate) fn drag_simulation_slider(&mut self, control: &str, fraction: f32) {
        let track = self.inspector_rect(&format!("inspector.simulation.{control}"));
        let y = track.center().y;
        let to = pos2(track.left() + track.width() * fraction.clamp(0.0, 1.0), y);
        self.drag(pos2(track.center().x, y), to);
    }

    fn inspector_rect(&self, test_id: &str) -> Rect {
        let inspector = self.inspector();
        let rect = inspector
            .document
            .node_rect(inspector.find(test_id))
            .expect("the control was not laid out");
        rect.scaled(crate::inspector::scale(&self.context))
    }

    pub(crate) fn theme_option_center(&self, index: usize) -> Pos2 {
        let option = self
            .inspector()
            .option_node("inspector.simulation.theme", index);
        self.node_center(option)
    }

    pub(crate) fn performance_panel_visible(&self) -> bool {
        let inspector = self.inspector();
        inspector
            .document
            .find_test_id("inspector.performance")
            .and_then(|panel| inspector.document.node_rect(panel))
            .is_some()
    }

    fn tab_node(&self, index: usize) -> NodeId {
        self.inspector().option_node("inspector.tabs", index)
    }

    fn inspector_center(&self, test_id: &str) -> Pos2 {
        self.node_center(self.inspector().find(test_id))
    }

    pub(crate) fn touch_emulation(&self) -> bool {
        self.context.touch_emulation()
    }

    fn node_center(&self, id: NodeId) -> Pos2 {
        let center = self
            .inspector()
            .document
            .node_rect(id)
            .expect("the row was not laid out")
            .center();
        let scale = crate::inspector::scale(&self.context);
        pos2(center.x * scale, center.y * scale)
    }
}

fn key_event(key: Key, pressed: bool, modifiers: Modifiers) -> Event {
    Event::Key {
        key,
        pressed,
        repeat: false,
        modifiers,
    }
}

pub(crate) fn with_installed<R>(document: &mut Document, f: impl FnOnce(&mut Document) -> R) -> R {
    crate::reactive::with_reactive_scope(document, || crate::reactive::with_document(f))
}

#[component]
pub(crate) fn MenuRegion() -> NodeId {
    view! {
        <Frame width=120.0 height=60.0 color=Color32::from_gray(80) radius=4 />
    }
}

#[component]
pub(crate) fn ButtonFace(label: String) -> NodeId {
    view! {
        <Frame color=Color32::from_gray(60) radius=4 padding_horizontal=20.0 padding_vertical=12.0>
            <Text string={label} font_size=14.0 color=Color32::WHITE />
        </Frame>
    }
}

#[component]
pub(crate) fn LabelledButton(label: String, on_click: ClickCallback) -> NodeId {
    view! {
        <unstyled::Button on_click={move || on_click.call()}>
            <ButtonFace label />
        </unstyled::Button>
    }
}

#[component]
pub(crate) fn PanZoomStage(
    view: crate::reactive::Prop<unstyled::PanZoomView>,
    on_change: crate::reactive::Callback<unstyled::PanZoomView>,
) -> NodeId {
    view! {
        <unstyled::PanZoom @test_id="stage" view on_change={move |view| on_change.call(view)}>
            {move |handle: unstyled::PanZoomHandle| {
                let unstyled::PanZoomHandle { view, .. } = handle;
                view! {
                    <Canvas view>
                        <CanvasItem @test_id="item" x=0.0 y=0.0 width=100.0 height=50.0 />
                    </Canvas>
                }
            }}
        </unstyled::PanZoom>
    }
}

pub(crate) struct VirtualScroll {
    pub(crate) document: Document,
    pub(crate) scroll: NodeId,
    pub(crate) list: NodeId,
}

pub(crate) fn virtual_list(built: &Rc<RefCell<Vec<usize>>>) -> VirtualScroll {
    let (scroll, list) = (NodeRef::new(), NodeRef::new());
    let sink = built.clone();
    let document = build({
        let (scroll, list) = (scroll.clone(), list.clone());
        move || {
            view! {
                <List spacing=0.0>
                    <Offset @sizing=ItemSize::Percent(100.0) @node_ref=&scroll>
                        <VirtualList
                            @node_ref=&list
                            keys={indices(VIRTUAL_ITEM_COUNT)}
                            item_size=VIRTUAL_ITEM_HEIGHT
                        >
                            {move |index: usize| {
                                sink.borrow_mut().push(index);
                                view! {
                                    <Frame
                                        padding_horizontal=0.0
                                        padding_vertical={VIRTUAL_ITEM_HEIGHT / 2.0}
                                    >
                                        <Spacer />
                                    </Frame>
                                }
                            }}
                        </VirtualList>
                    </Offset>
                </List>
            }
        }
    });
    VirtualScroll {
        document,
        scroll: scroll.get(),
        list: list.get(),
    }
}

pub(crate) struct KeyedVirtualScroll {
    pub(crate) document: Document,
    pub(crate) scroll: NodeId,
    pub(crate) list: NodeId,
    pub(crate) set_keys: crate::reactive::WriteSignal<Vec<usize>>,
}

pub(crate) fn keyed_virtual_list(built: &Rc<RefCell<Vec<usize>>>) -> KeyedVirtualScroll {
    let (keys, set_keys) = create_signal(indices(VIRTUAL_ITEM_COUNT));
    let (scroll, list) = (NodeRef::new(), NodeRef::new());
    let sink = built.clone();
    let document = build({
        let (scroll, list) = (scroll.clone(), list.clone());
        move || {
            view! {
                <List spacing=0.0>
                    <Offset @sizing=ItemSize::Percent(100.0) @node_ref=&scroll>
                        <VirtualList @node_ref=&list keys={keys} item_size=VIRTUAL_ITEM_HEIGHT>
                            {move |key: usize| {
                                sink.borrow_mut().push(key);
                                view! {
                                    <Frame height=VIRTUAL_ITEM_HEIGHT>
                                        <Spacer />
                                    </Frame>
                                }
                            }}
                        </VirtualList>
                    </Offset>
                </List>
            }
        }
    });
    KeyedVirtualScroll {
        document,
        scroll: scroll.get(),
        list: list.get(),
        set_keys,
    }
}

pub(crate) struct HelloColumn {
    pub(crate) document: Document,
    pub(crate) padding: NodeId,
    pub(crate) text: NodeId,
}

pub(crate) fn hello_column() -> HelloColumn {
    let (padding, text) = (NodeRef::new(), NodeRef::new());
    let document = build({
        let (padding, text) = (padding.clone(), text.clone());
        move || {
            view! {
                <List spacing=0.0>
                    <Frame @node_ref=&padding padding_horizontal=4.0 padding_vertical=4.0>
                        <Text @node_ref=&text string="Hello" font_size=14.0 color=Color32::WHITE />
                    </Frame>
                </List>
            }
        }
    });
    HelloColumn {
        document,
        padding: padding.get(),
        text: text.get(),
    }
}

pub(crate) struct StackedPanels {
    pub(crate) document: Document,
    pub(crate) upper: NodeId,
    pub(crate) lower: NodeId,
}

pub(crate) struct ThreePanels {
    pub(crate) document: Document,
    pub(crate) list: NodeId,
    pub(crate) top: NodeId,
    pub(crate) middle: NodeId,
    pub(crate) bottom: NodeId,
}

pub(crate) fn three_panels() -> ThreePanels {
    let list = NodeRef::new();
    let (top, middle, bottom) = (NodeRef::new(), NodeRef::new(), NodeRef::new());
    let document = build({
        let list = list.clone();
        let (top, middle, bottom) = (top.clone(), middle.clone(), bottom.clone());
        move || {
            view! {
                <List @node_ref=&list spacing=0.0>
                    <Frame @node_ref=&top height=100.0 color=Color32::WHITE radius=0 />
                    <Frame @node_ref=&middle height=100.0 color={Color32::from_gray(40)} radius=0 />
                    <Frame @node_ref=&bottom height=100.0 color={Color32::from_gray(80)} radius=0 />
                </List>
            }
        }
    });
    ThreePanels {
        document,
        list: list.get(),
        top: top.get(),
        middle: middle.get(),
        bottom: bottom.get(),
    }
}

pub(crate) fn stacked_panels() -> StackedPanels {
    let (upper, lower) = (NodeRef::new(), NodeRef::new());
    let document = build({
        let (upper, lower) = (upper.clone(), lower.clone());
        move || {
            view! {
                <List spacing=0.0>
                    <Frame @node_ref=&upper height=100.0 color=Color32::WHITE radius=0 />
                    <Frame @node_ref=&lower height=100.0 color={Color32::from_gray(40)} radius=0 />
                </List>
            }
        }
    });
    StackedPanels {
        document,
        upper: upper.get(),
        lower: lower.get(),
    }
}

pub(crate) fn flashed(output: &crate::FrameOutput, bounds: Rect, color: Color32) -> bool {
    output.shapes().iter().any(|shape| {
        matches!(
            shape,
            crate::painter::Shape::Rect {
                rect,
                stroke_width,
                color: painted,
                ..
            } if *rect == bounds
                && *stroke_width > 0.0
                && painted.to_array()[..3] == color.to_array()[..3]
        )
    })
}

pub(crate) fn indices(count: usize) -> Vec<usize> {
    (0..count).collect()
}

pub(crate) fn text_of(document: &Document, id: NodeId) -> &str {
    document.text(id)
}

pub(crate) fn dock_of(tabs: usize) -> (Document, NodeId) {
    let dock = NodeRef::new();
    let built = dock.clone();
    let tabs: Vec<unstyled::TabId> = (1..=tabs)
        .map(|index| unstyled::TabId::new(index as u64))
        .collect();
    let document = build(move || {
        let (state, set_state) = create_signal(unstyled::DockState::new(tabs));
        view! {
            <styled::DockArea
                @node_ref=&built
                state={state}
                title={Func::new(|tab: unstyled::TabId| format!("Tab {}", tab.value()))}
                on_change={move |next: unstyled::DockState| set_state.set(next)}
                on_close={move |_: unstyled::TabId| {}}
            >
                {move |tab: unstyled::TabId| view! {
                    <Frame @test_id={format!("content.{}", tab.value())} />
                }}
            </styled::DockArea>
        }
    });
    (document, dock.get())
}

pub(crate) fn text_within(document: &Document, root: NodeId, text: &str) -> Option<NodeId> {
    if document.node_kind(root) == "text"
        && document.text(root) == text
        && document.node_rect(root).is_some()
    {
        return Some(root);
    }
    document
        .children(root)
        .into_iter()
        .find_map(|child| text_within(document, child, text))
}

pub(crate) fn dock_tab(document: &Document, dock: NodeId, title: &str) -> NodeId {
    text_within(document, dock, title).unwrap_or_else(|| panic!("the dock shows {title}"))
}

pub(crate) fn drag_with(harness: &mut Harness, from: Pos2, to: Pos2, modifiers: Modifiers) {
    harness.frame(vec![Event::PointerMoved(from), Event::Modifiers(modifiers)]);
    harness.frame(vec![Event::PointerButton {
        pos: from,
        button: PointerButton::Primary,
        pressed: true,
        modifiers,
    }]);
    harness.frame(vec![Event::PointerMoved(to), Event::Modifiers(modifiers)]);
    harness.frame(vec![Event::PointerButton {
        pos: to,
        button: PointerButton::Primary,
        pressed: false,
        modifiers,
    }]);
}

pub(crate) fn toolbar_of<const N: usize>(
    controls: impl FnOnce() -> [NodeId; N],
) -> (Document, [NodeId; N]) {
    let built = Rc::new(Cell::new(None));
    let sink = built.clone();
    let document = build(move || {
        let nodes = controls();
        sink.set(Some(nodes));
        view! {
            <List spacing=8.0>
                <ForEach keys={indices(N)}>{move |index: usize| nodes[index]}</ForEach>
            </List>
        }
    });
    let nodes = built.get().expect("the toolbar was built");
    (document, nodes)
}

use crate::node::{Element, InteractInput, NodeMap};
use crate::painter::Painter;
use std::any::Any;
use std::time::{Duration, Instant};

struct Counted {
    inner: Box<dyn Element>,
    layouts: Rc<Cell<usize>>,
    paints: Rc<Cell<usize>>,
    measures: Rc<Cell<usize>>,
}

impl Element for Counted {
    fn measure(&self, doc: &mut Document, painter: &Painter, available: Vec2) -> Vec2 {
        self.measures.set(self.measures.get() + 1);
        self.inner.measure(doc, painter, available)
    }

    fn layout(
        &mut self,
        doc: &mut Document,
        painter: &Painter,
        rect: Rect,
        out: &mut NodeMap<Rect>,
    ) {
        self.layouts.set(self.layouts.get() + 1);
        self.inner.layout(doc, painter, rect, out);
    }

    fn paint(&self, doc: &Document, painter: &Painter, rects: &NodeMap<Rect>, rect: Rect) {
        self.paints.set(self.paints.get() + 1);
        self.inner.paint(doc, painter, rects, rect);
    }

    fn captures(&mut self, doc: &mut Document, pos: Pos2, rect: Rect) -> bool {
        self.inner.captures(doc, pos, rect)
    }

    fn interact(
        &mut self,
        doc: &mut Document,
        painter: &Painter,
        input: &InteractInput,
        id: NodeId,
        rect: Rect,
        focus_target: &mut Option<NodeId>,
        children: &mut Vec<NodeId>,
    ) {
        self.inner
            .interact(doc, painter, input, id, rect, focus_target, children)
    }

    fn children(&self) -> Vec<NodeId> {
        self.inner.children()
    }
    fn kind(&self) -> &'static str {
        self.inner.kind()
    }
    fn as_any(&self) -> &dyn Any {
        self.inner.as_any()
    }
    fn as_any_mut(&mut self) -> &mut dyn Any {
        self.inner.as_any_mut()
    }
}

struct Counts {
    layouts: Rc<Cell<usize>>,
    paints: Rc<Cell<usize>>,
    measures: Rc<Cell<usize>>,
}

const BLINK: Duration = Duration::from_millis(530);

fn blinking() -> crate::reactive::Draw {
    Rc::new(|painter: &crate::painter::Painter, rect: Rect| {
        painter.rect_filled(rect, 0.0, Color32::WHITE);
        painter.ctx().request_repaint_after(BLINK);
    })
}

fn still() -> crate::reactive::Draw {
    Rc::new(|painter: &crate::painter::Painter, rect: Rect| {
        painter.rect_filled(rect, 0.0, Color32::WHITE);
    })
}

fn counted(document: &mut Document, node: NodeId) -> (Rc<Cell<usize>>, Rc<Cell<usize>>) {
    let counts = counted_with_measures(document, node);
    (counts.layouts, counts.paints)
}

fn counted_with_measures(document: &mut Document, node: NodeId) -> Counts {
    let counts = Counts {
        layouts: Rc::new(Cell::new(0)),
        paints: Rc::new(Cell::new(0)),
        measures: Rc::new(Cell::new(0)),
    };
    let inner = document.arena.take(node);
    document.arena.put_back(
        node,
        Box::new(Counted {
            inner,
            layouts: counts.layouts.clone(),
            paints: counts.paints.clone(),
            measures: counts.measures.clone(),
        }),
    );
    counts
}
mod a_clean_panel_is_not_laid_out_again_when_the_one_beside_it_changes;
mod a_clean_sibling_keeps_its_measurement_when_the_one_beside_it_changes;
mod a_click_handler_can_mutate_the_tree_in_the_current_frame;
mod a_panel_between_two_damaged_ones_is_left_alone;
mod a_panel_taken_out_of_its_list_gives_up_its_rectangle_and_damages_it;
mod a_scroll_only_re_measures_the_row_that_changed;
mod accordion_headers_are_keyboard_operable_and_skip_collapsed_content;
mod activation_requires_a_matching_release_and_escape_cancels_it;
mod clicking_a_choice_keeps_keyboard_focus_on_the_selected_option;
mod copy_and_cut_export_only_selected_text_and_cut_can_be_undone;
mod empty_choices_and_invalid_selection_do_not_break_tab_navigation;
mod every_styled_interactive_control_paints_a_keyboard_focus_ring;
mod focus_loss_and_hidden_content_cancel_keyboard_activation;
mod hover_only_repaints_when_its_handler_changes_a_node;
mod key_handlers_can_move_focus_and_change_their_tab_stop;
mod key_repeats_and_shortcut_modifiers_do_not_accidentally_activate_controls;
mod keyboard_scrolling_reaches_virtual_items_and_endpoints;
mod list_rows_and_pressables_activate_from_the_keyboard;
mod listbox_navigation_reveals_options_inside_a_tall_scroll_item;
mod listbox_typeahead_matches_prefixes_and_cycles_repeated_letters;
mod losing_window_focus_cancels_a_held_activation_key;
mod performance_measurements_report_what_the_frame_reused;
mod radio_groups_select_with_space_and_arrows_without_leaving_the_group;
mod resizing_scaling_and_replacing_the_root_invalidate_the_cache;
mod slider_home_end_and_page_keys_clamp_at_the_bounds;
mod space_toggles_checkboxes_switches_and_toggle_buttons;
mod tabbing_to_an_offscreen_control_reveals_it;
mod tabs_have_one_tab_stop_and_wrap_with_arrow_keys;
mod text_laid_out_to_an_alignment_indents_each_line_to_it;
mod unchanged_input_reuses_layout_and_paint;

mod a_bare_separator_rules_across_the_column_it_sits_in;
mod a_frame_width_bound_to_a_signal_measures_intrinsically_once_it_clears;
mod a_narrow_window_shows_the_inspector_below_a_tab_bar;
mod a_separator_keeps_the_length_it_is_given_where_its_row_centres_it;
mod a_vertical_separator_rules_down_the_row_it_sits_in;
mod a_window_without_room_for_the_app_beside_the_inspector_uses_the_tab_bar;
mod open_inspector_opens_the_inspector_from_inside_the_document;
mod picking_in_a_narrow_window_returns_to_the_inspector;
mod the_app_tab_shows_the_document_below_the_tab_bar;
mod the_inspector_shows_the_renderer_the_host_reports;
mod unused_navigation_keys_scroll_the_nearest_ancestor;

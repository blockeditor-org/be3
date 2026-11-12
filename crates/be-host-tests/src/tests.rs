use crate::{HostTest, Server};

mod a_beui_demos_dock_joins_the_workspace_dock_as_a_locked_group;
mod a_block_added_from_the_files_is_named_in_its_row;
mod a_canvas_in_a_canvas_keeps_to_its_card_and_closes_when_pressed_outside;
mod a_canvas_in_a_canvas_shows_its_tool_bar_in_place_of_the_outer_ones;
mod a_canvas_pans_with_the_wheel_and_drags_and_zooms_with_ctrl;
mod a_canvas_popped_out_into_a_window_hides_the_files_behind_it;
mod a_canvas_tool_bar_dragged_by_its_grip_goes_where_it_is_dropped_and_stays_there;
mod a_canvas_tool_bar_goes_where_its_grip_menu_puts_it;
mod a_canvas_tool_bar_popped_out_lifts_out_of_its_window_and_follows_its_grip;
mod a_checklist_on_a_canvas_works_in_place_and_zooms_with_it;
mod a_contact_link_asks_before_connecting_two_accounts;
mod a_control_focused_in_the_desktops_apps_panel_reads_as_focused;
mod a_deleted_joined_block_says_so_on_each_of_its_tabs;
mod a_demo_tab_unlocked_from_its_grip_moves_beside_the_files_with_its_page;
mod a_desktop_window_moves_by_super_drag_and_keeps_its_bar_on_the_screen;
mod a_device_restarted_offline_catches_up_once_it_reconnects;
mod a_device_restarted_offline_reopens_the_document_it_had_open;
mod a_device_restarted_offline_still_lists_a_block_another_device_made;
mod a_device_restarted_offline_still_lists_a_block_it_made;
mod a_device_that_cannot_reach_its_server_opens_its_files_and_edits_at_once;
mod a_device_that_loaded_my_files_adds_to_it_offline_and_the_other_sees_it_once_it_reconnects;
mod a_dock_grip_menu_row_loses_its_highlight_when_the_pointer_leaves_the_menu;
mod a_drag_across_an_empty_map_pans_it;
mod a_files_row_menu_escapes_the_files_where_drive_finds_it;
mod a_game_on_a_canvas_takes_its_first_drag_as_a_move_and_hands_its_bars_back;
mod a_joined_blocks_tab_opens_its_details_and_palette;
mod a_joined_blocks_tabs_carry_its_menu;
mod a_joined_demos_tab_bodies_are_in_the_accessibility_tree;
mod a_local_account_needs_only_a_display_name;
mod a_menu_from_a_zoomed_out_checklist_on_a_canvas_escapes_its_card_at_full_size;
mod a_new_folder_opens_its_editor;
mod a_new_image_asks_for_its_file_in_the_image_editors_own_dialog;
mod a_new_text_block_opens_in_a_tab_beside_the_files;
mod a_note_made_while_a_device_is_offline_reaches_it_once_it_reconnects;
mod a_pan_and_zoom_editor_on_a_canvas_takes_the_frame_in_place_and_pans_with_it;
mod a_pen_draws_a_line_on_a_canvas_with_the_pen_tool;
mod a_personal_organization_can_create_an_agent;
mod a_popup_a_joined_demo_tab_places_over_its_own_window_opens_over_that_tab;
mod a_popup_in_space_closes_on_a_press_in_another_panel;
mod a_presentation_menu_offers_new_slides_first;
mod a_presented_slide_takes_clicks_and_leaves_the_keys_to_the_deck;
mod a_press_on_the_files_add_button_closes_a_canvas_menu_and_opens_theirs;
mod a_press_outside_a_menu_and_its_submenu_closes_both_and_reaches_what_it_landed_on;
mod a_second_click_on_a_zoomed_workspaces_add_button_closes_its_menu;
mod a_second_finger_turns_a_pen_stroke_into_a_pinch;
mod a_session_in_the_desktop_gives_each_tab_only_its_own_blocks_actions;
mod a_setting_opens_from_the_settings_tab_beside_the_files;
mod a_settings_page_and_the_settings_list_show_the_same_block_side_by_side;
mod a_slides_canvas_bar_shares_the_top_band_with_the_presentation_toolbar;
mod a_slides_preview_catches_up_with_quick_changes_after_a_pause;
mod a_tab_keeps_its_view_through_a_resize;
mod a_tab_popped_out_in_space_is_a_panel_of_its_own;
mod a_text_reopened_after_going_back_or_a_restart_keeps_its_scroll_and_caret;
mod a_tooltip_from_the_files_reaches_past_them;
mod a_window_dragged_over_the_desktops_bars_passes_beneath_them;
mod a_window_over_a_tab_docked_on_the_desktop_draws_above_it;
mod adding_an_account_chooses_a_local_or_custom_server;
mod an_active_child_pans_in_the_frame_its_canvas_pans;
mod an_edit_made_offline_reads_as_kept_on_the_device_and_syncs_without_an_error;
mod an_invitation_shows_the_link_to_send_the_invited_person;
mod an_offline_device_refuses_to_move_a_block_into_a_folder_it_never_opened;
mod an_offline_device_says_a_block_it_never_opened_is_not_on_it_yet;
mod an_offline_device_that_never_opened_my_files_is_told_it_cannot_add_to_it;
mod choosing_a_scale_draws_the_host_larger;
mod choosing_another_workspace_on_the_add_pickers_location_page_moves_it_there;
mod closing_a_demos_block_tab_closes_the_tabs_its_dock_put_in_the_workspace;
mod closing_a_tab_shuts_its_editor_down;
mod dragging_the_workspace_splitter_draws_the_files_at_their_new_width_in_the_same_frame;
mod fit_on_an_active_child_fits_its_content_in_its_view;
mod going_back_from_the_add_pickers_location_page_draws_the_menu_and_its_card_in_the_same_frame;
mod going_back_from_the_add_pickers_location_page_grows_the_menu_again;
mod in_a_narrow_pane_a_deleted_block_puts_restore_below_its_notice;
mod killing_the_desktop_shell_shows_the_session_picker;
mod leaving_a_workspace_someone_shared_asks_first;
mod more_blocks_from_the_add_menu_in_space_opens_the_search;
mod moving_the_pointer_over_an_empty_part_of_the_workspace_steps_no_other_app;
mod my_files_offers_no_delete;
mod on_a_narrow_window_the_add_menu_from_the_files_rises_as_a_sheet;
mod on_a_phone_a_demos_dock_tabs_are_tabs_of_the_workspace;
mod on_a_phone_a_held_file_rises_its_menu_as_a_sheet_over_the_whole_host;
mod on_a_phone_an_editor_stacks_over_the_files_with_its_panes_gathered_below_its_stage;
mod on_a_phone_back_leaves_each_document_opened_from_the_files;
mod on_a_phone_tapping_a_blocks_name_renames_it_in_a_sheet;
mod on_a_phone_the_files_show_what_the_device_keeps_after_its_connection_drops;
mod on_a_phone_the_host_menu_rises_as_a_sheet_it_draws_itself;
mod presenting_a_deck_again_and_again_adds_no_blocks;
mod presenting_a_deck_fills_the_screen_until_escape;
mod pressing_the_files_after_a_text_block_moves_the_focus_to_them;
mod quick_open_finds_a_word_typed_in_a_text_and_opens_it;
mod quick_open_goes_to_a_file_and_the_palette_offers_the_workspaces_and_hosts_actions;
mod removing_a_local_account_asks_first_and_names_what_is_lost;
mod reopening_a_block_keeps_the_view_the_person_moved;
mod resizing_the_window_draws_the_workspace_and_its_files_at_the_new_size_in_the_same_frame;
mod revision_history_previews_and_restores_a_text_file;
mod the_add_menu_fits_a_short_window_and_scrolls_inside_itself;
mod the_add_menu_opens_at_the_files_button_that_asked;
mod the_add_menu_reaches_past_narrow_files_and_closes_on_a_press_outside;
mod the_add_picker_settles_at_a_fractional_scale;
mod the_canvas_dock_opens_the_add_menu_at_its_block_button;
mod the_canvas_menu_ticks_its_grid_and_profile_in_submenus;
mod the_canvas_zoom_bar_floats_at_any_corner_and_stays_there;
mod the_desktop_shell_opens_and_closes_cleanly;
mod the_desktops_bars_hold_their_place_empty_and_their_grips_open_their_menu;
mod the_files_new_menu_row_loses_its_highlight_when_the_pointer_leaves;
mod the_host_menu_opens_under_its_button;
mod the_host_opens_its_files_and_closes_cleanly;
mod the_host_palette_lists_a_text_on_a_canvas_before_the_canvas;
mod the_host_palette_lists_the_desktops_window_keys;
mod the_inspector_chord_in_the_more_blocks_search_inspects_the_workspace;
mod the_inspector_opens_in_the_focused_app_over_its_view_and_keeps_its_children;
mod the_members_dialog_lists_an_organizations_members_and_their_roles;
mod the_more_card_from_the_files_reaches_over_the_block_beside_them;
mod the_more_card_hangs_on_its_tab_and_leaves_the_files_usable;
mod the_organizations_screen_takes_an_invite_link;
mod the_rename_card_from_the_files_reaches_past_them;
mod the_rename_card_takes_the_keyboard_with_its_name_selected;
mod the_storage_page_says_what_the_server_counts;
mod the_text_settings_point_at_the_tab_from_its_menu_and_the_palette;
mod the_workspace_splitter_reaches_over_the_files_and_its_tabs_scroll_with_the_wheel;
mod two_canvas_tabs_share_one_popped_out_tool_bar_that_follows_the_tab_shown;
mod typing_after_a_new_session_opens_reaches_the_session;
mod typing_after_a_pause_undoes_as_its_own_step;
mod typing_as_the_connection_silently_drops_leaves_the_host_settled;
mod typing_in_the_add_menu_from_the_files_reaches_it_straight_away;
mod typing_right_after_adding_notes_reaches_the_notes;

fn add_to_my_files(host: &mut HostTest, label: &str) {
    start_adding_to_my_files(host, label);
    host.key("Enter");
    host.gone(&format!("TextInput \"Name\" value=\"{label}\""));
    host.wait(&format!("TreeItem \"{label}\""));
}

fn start_adding_to_my_files(host: &mut HostTest, label: &str) {
    open_add_to_my_files(host);
    choose_type(host, label);
    host.wait(&format!("Pane \"{label}\""));
    host.wait(&format!("TextInput \"Name\" value=\"{label}\" focused"));
}

fn drag_bar(host: &mut HostTest, grip: &str, to: (f32, f32)) {
    let from = host.place(grip);
    let start = (from.0 + from.2 / 2.0, from.1 + from.3 / 2.0);
    host.run(&["down", grip]);
    for step in 1..=4 {
        let share = step as f32 / 4.0;
        let x = start.0 + (to.0 - start.0) * share;
        let y = start.1 + (to.1 - start.1) * share;
        host.run(&["move", &format!("{x},{y}")]);
    }
    host.run(&["pause", "200"]);
    host.run(&["up"]);
    host.run(&["pause", "1000"]);
}

fn add_to_canvas(host: &mut HostTest, label: &str) {
    host.click("#infinite-canvas.dock.block");
    choose_type(host, label);
    host.wait("GenericContainer #infinite-canvas.entity.");
    host.gone("#editor.loading");
    host.run(&["settle"]);
}

fn choose_type(host: &mut HostTest, label: &str) {
    host.wait("#picker.more");
    let common = format!("MenuItem \"{label}\"");
    if host.shown(&common) {
        host.click(&common);
        host.gone("#picker.more");
        return;
    }
    host.click("#picker.more");
    host.wait("#picker.search");
    host.type_text(label);
    host.click(&format!("Button \"{label}\""));
    host.gone("#picker.search");
}

fn row_menu(host: &mut HostTest, title: &str) -> String {
    let tree = host.tree();
    let row = format!("TreeItem \"{title}\"");
    tree.lines()
        .skip_while(|line| !line.contains(&row))
        .skip(1)
        .take_while(|line| !line.contains("TreeItem "))
        .flat_map(str::split_whitespace)
        .find(|word| word.starts_with("#file-tree.") && word.ends_with(".more"))
        .unwrap_or_else(|| panic!("the file row {title:?} has no menu button in\n{tree}"))
        .to_owned()
}

fn open_beui_demo(host: &mut HostTest) {
    add_to_my_files(host, "beui demo");
    host.wait("Tab \"Components\"");
    host.wait("Tab \"Docking\"");
    host.gone("#editor.loading");
    host.run(&["settle"]);
}

fn open_add_to_my_files(host: &mut HostTest) {
    host.click(NEW);
    host.click(NEW_IN_MY_FILES);
}

const NEW: &str = "#file-tree.new";
const NEW_IN_MY_FILES: &str = "MenuItem \"New in My files…\"";

fn pan_zoom_world(host: &mut HostTest) -> (f32, f32, f32, f32) {
    let origin = host.place("#pan_zoom.card.0");
    let right = host.place("#pan_zoom.card.5");
    let scale = (right.0 + right.2 - origin.0) / 820.0;
    (origin.0, origin.1, 880.0 * scale, 580.0 * scale)
}

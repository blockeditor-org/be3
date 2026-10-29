use crate::AppTest;

mod a_beui_demos_dock_joins_the_workspace_dock_as_a_locked_group;
mod a_block_added_from_the_files_is_named_in_its_row;
mod a_canvas_in_a_canvas_keeps_to_its_card_and_closes_when_pressed_outside;
mod a_canvas_pans_with_the_wheel_and_drags_and_zooms_with_ctrl;
mod a_canvas_popped_out_into_a_window_hides_the_files_behind_it;
mod a_checklist_on_a_canvas_works_in_place_and_zooms_with_it;
mod a_contact_link_asks_before_connecting_two_accounts;
mod a_deleted_joined_block_says_so_on_each_of_its_tabs;
mod a_demo_tab_unlocked_from_the_workspace_dock_moves_beside_the_files_with_its_page;
mod a_drag_across_an_empty_map_pans_it;
mod a_files_row_menu_escapes_the_files_where_drive_finds_it;
mod a_joined_blocks_tab_opens_its_details_and_palette;
mod a_joined_blocks_tabs_carry_its_menu;
mod a_joined_demos_tab_bodies_are_in_the_accessibility_tree;
mod a_local_account_needs_only_a_display_name;
mod a_menu_from_a_zoomed_out_checklist_on_a_canvas_escapes_its_card_at_full_size;
mod a_new_folder_opens_its_editor;
mod a_new_image_asks_for_its_file_in_the_image_editors_own_dialog;
mod a_new_text_block_opens_in_a_tab_beside_the_files;
mod a_pen_draws_a_line_on_a_canvas_with_the_pen_tool;
mod a_personal_organization_can_create_an_agent;
mod a_pixel_art_on_a_canvas_takes_the_frame_in_place_and_pans_with_it;
mod a_popup_a_joined_demo_tab_places_over_its_own_window_opens_over_that_tab;
mod a_popup_in_space_closes_on_a_press_in_another_panel;
mod a_popups_own_popup_leaves_it_open_and_a_press_outside_closes_both;
mod a_presentation_menu_offers_new_slides_first;
mod a_presented_slide_takes_clicks_and_leaves_the_keys_to_the_deck;
mod a_press_in_the_files_closes_the_add_menu_a_canvas_opened;
mod a_press_outside_a_menu_and_its_submenu_closes_both_and_reaches_what_it_landed_on;
mod a_second_finger_turns_a_pen_stroke_into_a_pinch;
mod a_session_in_the_desktop_gives_each_tab_only_its_own_blocks_actions;
mod a_setting_opens_from_the_settings_tab_beside_the_files;
mod a_slides_preview_catches_up_with_quick_changes_after_a_pause;
mod a_tab_keeps_its_view_through_a_resize;
mod a_tab_popped_out_in_space_is_a_panel_of_its_own;
mod a_tooltip_from_the_files_reaches_past_them;
mod adding_an_account_chooses_a_local_or_custom_server;
mod an_active_child_pans_in_the_frame_its_canvas_pans;
mod an_invitation_shows_the_link_to_send_the_invited_person;
mod choosing_a_scale_draws_the_app_larger;
mod closing_a_demos_block_tab_closes_the_tabs_its_dock_put_in_the_workspace;
mod closing_a_tab_shuts_its_editor_down;
mod fit_on_an_active_child_fits_its_content_in_its_view;
mod leaving_a_workspace_someone_shared_asks_first;
mod more_blocks_from_the_add_menu_in_space_opens_the_search;
mod my_files_offers_no_delete;
mod on_a_narrow_window_the_add_menu_from_the_files_rises_as_a_sheet;
mod on_a_phone_a_demos_dock_tabs_are_tabs_of_the_workspace;
mod on_a_phone_a_held_file_rises_its_menu_as_a_sheet_over_the_whole_app;
mod presenting_a_deck_fills_the_screen_until_escape;
mod pressing_the_files_after_a_text_block_moves_the_focus_to_them;
mod reopening_a_block_keeps_the_view_the_person_moved;
mod revision_history_previews_and_restores_a_text_file;
mod the_add_menu_fits_a_short_window_and_scrolls_inside_itself;
mod the_add_menu_opens_at_the_files_button_that_asked;
mod the_app_menu_opens_under_its_button;
mod the_app_opens_its_files_and_closes_cleanly;
mod the_canvas_dock_opens_the_add_menu_at_its_block_button;
mod the_canvas_menu_ticks_its_grid_and_profile_in_submenus;
mod the_desktop_shell_opens_and_closes_cleanly;
mod the_members_dialog_lists_an_organizations_members_and_their_roles;
mod the_more_card_from_the_files_reaches_over_the_block_beside_them;
mod the_more_card_hangs_on_its_tab_and_leaves_the_files_usable;
mod the_organizations_screen_takes_an_invite_link;
mod the_rename_card_from_the_files_reaches_past_them;
mod typing_after_a_pause_undoes_as_its_own_step;

fn add_to_my_files(app: &mut AppTest, label: &str) {
    start_adding_to_my_files(app, label);
    app.key("Enter");
    app.gone(&format!("TextInput \"Name\" value=\"{label}\""));
    app.wait(&format!("TreeItem \"{label}\""));
}

fn start_adding_to_my_files(app: &mut AppTest, label: &str) {
    app.click(ADD_TO_MY_FILES);
    choose_type(app, label);
    app.wait(&format!("Pane \"{label}\""));
    app.wait(&format!("TextInput \"Name\" value=\"{label}\" focused"));
}

fn add_to_canvas(app: &mut AppTest, label: &str) {
    app.click("#infinite-canvas.dock.block");
    choose_type(app, label);
    app.wait("GenericContainer #infinite-canvas.entity.");
    app.gone("#editor.loading");
    app.run(&["settle"]);
}

fn choose_type(app: &mut AppTest, label: &str) {
    app.wait("#picker.more");
    let common = format!("MenuItem \"{label}\"");
    if app.shown(&common) {
        app.click(&common);
        app.gone("#picker.more");
        return;
    }
    app.click("#picker.more");
    app.wait("#picker.search");
    app.type_text(label);
    app.click(&format!("Button \"{label}\""));
    app.gone("#picker.search");
}

fn row_menu(app: &mut AppTest, title: &str) -> String {
    let tree = app.tree();
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

fn open_beui_demo(app: &mut AppTest) {
    app.click(ADD_TO_MY_FILES);
    choose_type(app, "beui demo");
    app.wait("Tab \"Components\"");
    app.wait("Tab \"Docking\"");
    app.gone("#editor.loading");
    app.key("Enter");
    app.run(&["settle"]);
}

const ADD_TO_MY_FILES: &str = "Button \"Add to My files\"";

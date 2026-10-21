use crate::AppTest;

mod a_block_added_from_the_files_is_named_in_its_row;
mod a_canvas_in_a_canvas_keeps_to_its_card_and_closes_when_pressed_outside;
mod a_canvas_pans_with_the_wheel_and_drags_and_zooms_with_ctrl;
mod a_checklist_on_a_canvas_works_in_place_and_zooms_with_it;
mod a_contact_link_asks_before_connecting_two_accounts;
mod a_drag_across_an_empty_map_pans_it;
mod a_local_account_needs_only_a_display_name;
mod a_new_beui_demo_opens_its_gallery_in_a_tab;
mod a_new_folder_opens_its_editor;
mod a_new_text_block_opens_in_a_tab_beside_the_files;
mod a_pen_draws_a_line_on_a_canvas_with_the_pen_tool;
mod a_personal_organization_can_create_an_agent;
mod a_pixel_art_on_a_canvas_takes_the_frame_in_place_and_pans_with_it;
mod a_presentation_menu_offers_new_slides_first;
mod a_second_finger_turns_a_pen_stroke_into_a_pinch;
mod a_setting_opens_from_the_settings_tab_beside_the_files;
mod adding_an_account_chooses_a_local_or_custom_server;
mod an_invitation_shows_the_link_to_send_the_invited_person;
mod choosing_a_scale_draws_the_app_larger;
mod closing_a_tab_shuts_its_editor_down;
mod leaving_a_workspace_someone_shared_asks_first;
mod presenting_a_deck_fills_the_screen_until_escape;
mod pressing_the_files_after_a_text_block_moves_the_focus_to_them;
mod revision_history_previews_and_restores_a_text_file;
mod the_add_menu_fits_a_short_window_and_scrolls_inside_itself;
mod the_add_menu_opens_at_the_files_button_that_asked;
mod the_app_menu_opens_under_its_button;
mod the_app_opens_its_files_and_closes_cleanly;
mod the_desktop_shell_opens_and_closes_cleanly;
mod the_members_dialog_lists_an_organizations_members_and_their_roles;
mod the_more_card_from_the_files_reaches_over_the_block_beside_them;
mod the_more_card_hangs_on_its_tab_and_leaves_the_files_usable;
mod the_organizations_screen_takes_an_invite_link;

fn add_to_my_files(app: &mut AppTest, label: &str) {
    app.click(ADD_TO_MY_FILES);
    choose_type(app, label);
    app.wait(&format!("Pane \"{label}\""));
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

const ADD_TO_MY_FILES: &str = "Button \"Add to My files\"";

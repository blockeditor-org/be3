use crate::AppTest;

mod a_canvas_in_a_canvas_keeps_to_its_card_and_closes_when_pressed_outside;
mod a_canvas_pans_with_the_wheel_and_drags_and_zooms_with_ctrl;
mod a_checklist_on_a_canvas_works_in_place_and_zooms_with_it;
mod a_contact_link_asks_before_connecting_two_accounts;
mod a_local_account_needs_only_a_display_name;
mod a_new_beui_demo_opens_its_gallery_in_a_tab;
mod a_new_folder_opens_its_editor;
mod a_new_text_block_opens_in_a_tab_beside_the_files;
mod a_pen_draws_a_line_on_a_canvas_with_the_pen_tool;
mod a_personal_organization_can_create_an_agent;
mod a_pixel_art_on_a_canvas_takes_the_frame_in_place_and_pans_with_it;
mod a_setting_opens_from_the_settings_tab_beside_the_files;
mod adding_an_account_chooses_a_local_or_custom_server;
mod an_invitation_shows_the_link_to_send_the_invited_person;
mod choosing_a_scale_draws_the_app_larger;
mod closing_a_tab_shuts_its_editor_down;
mod leaving_a_workspace_someone_shared_asks_first;
mod pressing_the_files_after_a_text_block_moves_the_focus_to_them;
mod revision_history_previews_and_restores_a_text_file;
mod the_add_block_dialog_takes_presses_over_an_open_file;
mod the_app_opens_its_files_and_closes_cleanly;
mod the_desktop_shell_opens_and_closes_cleanly;
mod the_members_dialog_lists_an_organizations_members_and_their_roles;
mod the_organizations_screen_takes_an_invite_link;

fn add_to_my_files(app: &mut AppTest, label: &str) {
    app.click(ADD_TO_MY_FILES);
    app.click(&format!("Label value=\"{label}\""));
    app.wait(&format!("Pane \"{label}\""));
}

fn add_to_canvas(app: &mut AppTest, target: &str) {
    app.click("#infinite-canvas.dock.block");
    pick_tile(app, target);
    app.wait("GenericContainer #infinite-canvas.entity.");
    app.gone("#editor.loading");
    app.run(&["settle"]);
}

fn pick_tile(app: &mut AppTest, target: &str) {
    app.wait("Dialog \"Add block\"");
    let hidden = |app: &mut AppTest| {
        !app.tree()
            .lines()
            .any(|line| line.contains(target) && !line.contains("offscreen"))
    };
    for _ in 0..50 {
        if !hidden(app) {
            break;
        }
        app.run(&["wheel", "Dialog \"Add block\"", "3"]);
    }
    if hidden(app) {
        let tree = app.tree();
        panic!("scrolling the add-block dialog never showed {target:?}:\n{tree}");
    }
    app.click(target);
    app.gone("Dialog \"Add block\"");
}

const ADD_TO_MY_FILES: &str = "Button \"Add to My files\"";

const CANVAS_TILE: &str = "#picker.tile.696e6669-6e69-7465-2d63-616e76617301/main";

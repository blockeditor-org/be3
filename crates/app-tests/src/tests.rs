use crate::AppTest;

mod a_canvas_pans_with_the_wheel_and_drags_and_zooms_with_ctrl;
mod a_pixel_art_on_a_canvas_takes_the_frame_in_place_and_pans_with_it;
mod a_checklist_on_a_canvas_takes_the_frame_when_opened_and_gives_it_back;
mod a_new_text_block_opens_in_a_tab_beside_the_files;
mod closing_a_tab_shuts_its_editor_down;
mod the_add_block_dialog_takes_presses_over_an_open_file;
mod the_app_opens_its_files_and_closes_cleanly;
mod the_desktop_shell_opens_and_closes_cleanly;

fn add_root_block(app: &mut AppTest, label: &str) {
    app.click("#file-tree.add-root");
    app.click(&format!("Label value=\"{label}\""));
    app.wait(&format!("Pane \"{label}\""));
}

fn add_to_canvas(app: &mut AppTest, label: &str) {
    app.click("#infinite-canvas.dock.block");
    app.wait("Dialog \"Add block\"");
    let target = format!("Label value=\"{label}\"");
    while app
        .tree()
        .lines()
        .any(|line| line.contains(&target) && line.contains("offscreen"))
    {
        app.run(&["wheel", "Dialog \"Add block\"", "3"]);
    }
    app.click(&target);
    app.gone("Dialog \"Add block\"");
    app.wait("GenericContainer #infinite-canvas.entity.");
    app.gone("#editor.loading");
}

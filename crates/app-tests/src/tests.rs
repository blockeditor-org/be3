use crate::AppTest;

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

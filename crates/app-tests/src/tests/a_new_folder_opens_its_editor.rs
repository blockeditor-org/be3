use super::*;

#[test]
fn a_new_folder_opens_its_editor() {
    let mut app = AppTest::start();
    add_to_my_files(&mut app, "Folder");
    app.wait("TreeItem \"Folder\"");
    app.wait("#folder.empty");
    app.close();
}

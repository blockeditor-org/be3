use super::*;

#[test]
fn deleting_my_files_brings_the_same_one_back() {
    let mut app = AppTest::start();
    app.wait("TreeItem \"My files\"");
    app.run(&["click", "TreeItem \"My files\"", "right"]);
    app.click("#file-tree.section.delete");
    app.run(&["settle"]);
    app.wait("TreeItem \"My files\"");
    app.gone("TreeItem \"No workspaces yet\"");
    app.close();
}

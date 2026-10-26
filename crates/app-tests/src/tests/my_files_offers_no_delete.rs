use super::*;

#[test]
fn my_files_offers_no_delete() {
    let mut app = AppTest::start();
    app.wait("TreeItem \"My files\"");
    app.run(&["click", "TreeItem \"My files\"", "right"]);
    app.wait("#file-tree.section.rename");
    app.gone("#file-tree.section.delete");
    app.close();
}

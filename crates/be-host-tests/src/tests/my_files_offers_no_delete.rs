use super::*;

#[test]
fn my_files_offers_no_delete() {
    let mut host = HostTest::start();
    host.wait("TreeItem \"My files\"");
    host.run(&["click", "TreeItem \"My files\"", "right"]);
    host.wait("#file-tree.section.rename");
    host.gone("#file-tree.section.delete");
    host.close();
}

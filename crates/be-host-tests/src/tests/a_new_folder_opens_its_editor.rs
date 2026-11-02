use super::*;

#[test]
fn a_new_folder_opens_its_editor() {
    let mut host = HostTest::start();
    add_to_my_files(&mut host, "Folder");
    host.wait("#folder.empty");
    host.close();
}

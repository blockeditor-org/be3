use super::*;

#[test]
fn the_host_opens_its_files_and_closes_cleanly() {
    let mut host = HostTest::start(Options::default());
    host.wait("TreeItem \"Recently Deleted\"");
    host.snapshot("the_host_opens_with_its_files");
    host.close();
}

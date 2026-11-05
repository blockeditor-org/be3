use super::*;

#[test]
fn the_storage_page_says_what_the_server_counts() {
    let mut host = HostTest::start();
    host.click("Tab \"Settings\"");
    host.wait("#settings.storage");
    host.click("#settings.storage");
    host.wait("#settings.storage.used");
    host.wait("#settings.storage.up");
    host.gone("#settings.storage.unknown");
    host.close();
}

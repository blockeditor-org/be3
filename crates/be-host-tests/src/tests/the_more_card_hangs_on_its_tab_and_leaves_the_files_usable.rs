use super::*;

#[test]
fn the_more_card_hangs_on_its_tab_and_leaves_the_files_usable() {
    let mut host = HostTest::start(Options::default());
    add_to_my_files(&mut host, "Canvas");
    host.click("#infinite-canvas.dock.block");
    host.click("#picker.more");
    host.wait("#picker.search");
    host.type_text("check");
    host.wait("Button \"Checklist\"");
    host.snapshot("the_more_card_hangs_on_its_tab_and_leaves_the_files_usable");

    host.click("TreeItem \"Canvas\"");
    host.gone("#picker.search");
    assert!(
        host.shown("TreeItem \"Canvas\" selected"),
        "the files take a press beside the card, which closes it"
    );
    host.click("#infinite-canvas.dock.block");
    host.click("#picker.more");
    host.wait("#picker.search");
    host.type_text("check");
    host.click("Button \"Checklist\"");
    host.gone("#picker.search");
    host.wait("GenericContainer #infinite-canvas.entity.");
    host.close();
}

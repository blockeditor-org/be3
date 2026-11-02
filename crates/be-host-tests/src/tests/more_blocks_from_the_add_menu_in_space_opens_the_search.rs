use super::*;

#[test]
fn more_blocks_from_the_add_menu_in_space_opens_the_search() {
    let mut host = HostTest::start_in_space();
    host.wait(NEW);
    host.run(&["--into=space", "click", NEW]);
    host.run(&["--into=space", "click", NEW_IN_MY_FILES]);
    host.wait("#picker.more");
    host.key("Up");
    host.key("Up");
    host.key("Up");
    host.wait("MenuItem \"More blocks…\" focused");
    host.key("Enter");
    host.wait("#picker.search");
    host.type_text("check");
    host.wait("Button \"Checklist\"");
    let windows = host.run(&["windows"]);
    assert_eq!(
        windows
            .lines()
            .filter(|line| !line.contains("main"))
            .count(),
        0,
        "the card is drawn over the host's own panel, not in a panel of its own:\n{windows}"
    );
    host.snapshot("more_blocks_from_the_add_menu_in_space_opens_the_search");
    host.key("Tab");
    host.wait("Button \"Checklist\" focused");
    host.key("Enter");
    host.gone("#picker.search");
    host.wait("Pane \"Checklist\"");
    host.close();
}

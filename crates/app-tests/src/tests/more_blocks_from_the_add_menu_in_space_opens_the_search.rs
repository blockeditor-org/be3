use super::*;

#[test]
fn more_blocks_from_the_add_menu_in_space_opens_the_search() {
    let mut app = AppTest::start_in_space();
    app.wait(ADD_TO_MY_FILES);
    app.run(&["--into=space", "click", ADD_TO_MY_FILES]);
    app.wait("#picker.more");
    app.key("Up");
    app.key("Up");
    app.key("Up");
    app.wait("MenuItem \"More blocks…\" focused");
    app.key("Enter");
    app.wait("#picker.search");
    app.type_text("check");
    app.wait("Button \"Checklist\"");
    let windows = app.run(&["windows"]);
    assert_eq!(
        windows
            .lines()
            .filter(|line| !line.contains("main"))
            .count(),
        0,
        "the card is drawn over the app's own panel, not in a panel of its own:\n{windows}"
    );
    app.snapshot("more_blocks_from_the_add_menu_in_space_opens_the_search");
    app.key("Tab");
    app.wait("Button \"Checklist\" focused");
    app.key("Enter");
    app.gone("#picker.search");
    app.wait("Pane \"Checklist\"");
    app.close();
}

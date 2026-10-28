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
    let (_, _, search_width, _) = app.place("#picker.search");
    let windows = app.run(&["windows"]);
    let card = windows
        .lines()
        .filter(|line| !line.contains("main"))
        .filter_map(|line| line.split_whitespace().last()?.split_once('x'))
        .filter_map(|(width, _)| width.parse::<f32>().ok())
        .next()
        .unwrap_or_else(|| panic!("the card shows in a panel of its own:\n{windows}"));
    assert!(
        card > search_width,
        "the card's panel grows to hold the card, {card} wide around a search {search_width} wide"
    );
    app.snapshot("more_blocks_from_the_add_menu_in_space_opens_the_search");
    app.key("Tab");
    app.wait("Button \"Checklist\" focused");
    app.key("Enter");
    app.gone("#picker.search");
    app.wait("Pane \"Checklist\"");
    app.close();
}

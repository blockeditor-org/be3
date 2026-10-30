use super::*;

#[test]
fn typing_in_the_add_menu_from_the_files_reaches_it_straight_away() {
    let mut app = AppTest::start();
    app.click(ADD_TO_MY_FILES);
    app.wait("#picker.more");
    let tree = app.tree();
    let rows: Vec<&str> = tree
        .lines()
        .filter(|line| line.contains("MenuItem \""))
        .collect();
    let more = rows
        .iter()
        .position(|row| row.contains("MenuItem \"More blocks…\""))
        .unwrap_or_else(|| panic!("the menu offers more blocks:\n{tree}"));
    for _ in more..rows.len() {
        app.key("Up");
    }
    app.wait("MenuItem \"More blocks…\" focused");
    app.key("Enter");
    app.wait("#picker.search");
    app.type_text("check");
    app.wait("Button \"Checklist\"");
    app.key("Escape");
    app.gone("#picker.search");
    app.close();
}

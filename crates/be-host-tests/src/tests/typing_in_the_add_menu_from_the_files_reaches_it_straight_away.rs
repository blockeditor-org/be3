use super::*;

#[test]
fn typing_in_the_add_menu_from_the_files_reaches_it_straight_away() {
    let mut host = HostTest::start();
    open_add_to_my_files(&mut host);
    host.wait("#picker.more");
    let tree = host.tree();
    let rows: Vec<&str> = tree
        .lines()
        .filter(|line| line.contains("MenuItem \""))
        .collect();
    let more = rows
        .iter()
        .position(|row| row.contains("MenuItem \"More blocks…\""))
        .unwrap_or_else(|| panic!("the menu offers more blocks:\n{tree}"));
    for _ in more..rows.len() {
        host.key("Up");
    }
    host.wait("MenuItem \"More blocks…\" focused");
    host.key("Enter");
    host.wait("#picker.search");
    host.type_text("check");
    host.wait("Button \"Checklist\"");
    host.key("Escape");
    host.gone("#picker.search");
    host.close();
}

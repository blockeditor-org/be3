use super::*;

const MORE: &str = "#dock.editors.menu";
const LINE_NUMBERS: &str = "#dock.editors.menu.text.line-numbers";

/// Opens the tab's More menu and answers the ids of its rows, in order.
fn open_menu(host: &mut HostTest) -> Vec<String> {
    host.click(MORE);
    host.wait(LINE_NUMBERS);
    host.run(&["settle"]);
    host.tree()
        .split_whitespace()
        .filter(|word| word.starts_with("#dock.editors.menu."))
        .map(str::to_owned)
        .collect()
}

fn line_numbers_checked(host: &mut HostTest) -> bool {
    let tree = host.tree();
    let row = tree
        .lines()
        .find(|line| line.contains(LINE_NUMBERS))
        .unwrap_or_else(|| panic!("the menu has no Line numbers in\n{tree}"));
    assert!(
        row.contains("MenuItemCheckBox \"Line numbers\""),
        "Line numbers is a check box: {row}"
    );
    row.contains("toggled=True")
}

/// With two texts open in the editors pane, the More menu of the one shown lists its own commands
/// once (one app shows both texts, from one document), and its Line numbers check follows that
/// text's setting as it is turned on and off.
#[test]
fn the_text_menu_lists_each_item_once_and_checks_line_numbers() {
    let mut host = HostTest::start(Options::default());
    add_to_my_files(&mut host, "Text");
    add_to_my_files(&mut host, "Text");
    host.gone("#editor.loading");

    let rows = open_menu(&mut host);
    let mut seen = std::collections::HashSet::new();
    let repeated: Vec<&String> = rows.iter().filter(|row| !seen.insert(*row)).collect();
    assert!(
        repeated.is_empty(),
        "each row of the menu shows once, but {repeated:?} repeat in {rows:?}"
    );
    assert!(
        !line_numbers_checked(&mut host),
        "a new text hides its line numbers"
    );

    host.click(LINE_NUMBERS);
    host.gone(LINE_NUMBERS);
    host.run(&["settle"]);
    open_menu(&mut host);
    assert!(
        line_numbers_checked(&mut host),
        "Line numbers is checked once they show"
    );
    host.snapshot("the_text_menu_checks_line_numbers_once_they_show");

    host.click(LINE_NUMBERS);
    host.gone(LINE_NUMBERS);
    host.run(&["settle"]);
    open_menu(&mut host);
    assert!(
        !line_numbers_checked(&mut host),
        "Line numbers is unchecked once they are hidden again"
    );
    host.key("Escape");
    host.gone(LINE_NUMBERS);
    host.close();
}

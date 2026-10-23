use super::*;
use beui::Key;

#[test]
fn editing_a_tree_row_keeps_a_name_on_enter_or_a_click_away_and_escape_puts_it_back() {
    let mut test = alone(WIDE, Page::Tree);
    test.frame(Vec::new());
    let root = test.document().root().expect("the demo built a root");

    test.click("demo.tree.3.row");
    test.frame(Vec::new());
    press_key(&mut test, Key::F2);
    assert!(
        test.shows("demo.tree.3.rename"),
        "F2 edits the focused row's name in place"
    );
    let row = test.rect_of("demo.tree.3.row").height();
    let field = test.rect_of("demo.tree.3.rename").height();
    assert!(field <= row, "the field fits in the row, {field} in {row}");
    test.snapshot("a_tree_row_being_renamed_in_place");
    test.frame(vec![Input::Text("app.rs".to_owned())]);
    press_key(&mut test, Key::ArrowDown);
    assert!(
        test.shows("demo.tree.3.rename"),
        "the arrow keys stay in the field rather than walking the tree"
    );
    press_key(&mut test, Key::Enter);
    assert!(!test.shows("demo.tree.3.rename"), "Enter ends the edit");
    assert_eq!(
        showing(test.document(), root, "Renamed demo.rs to app.rs"),
        1,
        "Enter keeps the name typed over the whole of the old one"
    );

    test.click("demo.tree.5.more");
    test.frame(Vec::new());
    test.click("demo.tree.rename");
    test.frame(Vec::new());
    assert!(
        test.shows("demo.tree.5.rename"),
        "Rename in a row's menu edits it"
    );
    test.frame(vec![Input::Text("icon.png".to_owned())]);
    press_key(&mut test, Key::Escape);
    assert!(!test.shows("demo.tree.5.rename"), "Escape ends the edit");
    assert_eq!(
        showing(test.document(), root, "Kept logo.png"),
        1,
        "Escape puts the old name back"
    );

    press_key(&mut test, Key::F2);
    assert!(
        test.shows("demo.tree.5.rename"),
        "Escape hands the keyboard back to the row, so F2 edits it again"
    );
    test.frame(vec![Input::Text("   ".to_owned())]);
    press_key(&mut test, Key::Enter);
    assert_eq!(
        showing(test.document(), root, "Kept logo.png"),
        1,
        "a blank name changes nothing"
    );

    press_key(&mut test, Key::F2);
    test.frame(vec![Input::Text("icon.png".to_owned())]);
    test.click("demo.tree.8.row");
    test.frame(Vec::new());
    assert!(
        !test.shows("demo.tree.5.rename"),
        "clicking away ends the edit"
    );
    assert_eq!(
        showing(test.document(), root, "icon.png"),
        1,
        "clicking away keeps the name typed"
    );

    press_key(&mut test, Key::F2);
    test.frame(vec![Input::Text("Build.toml".to_owned())]);
    test.frame(vec![Input::Focus(false)]);
    test.frame(Vec::new());
    assert!(
        !test.shows("demo.tree.8.rename"),
        "the window losing the keyboard ends the edit"
    );
    assert_eq!(
        showing(test.document(), root, "Renamed Cargo.toml to Build.toml"),
        1,
        "and keeps the name typed, as clicking away does"
    );
}

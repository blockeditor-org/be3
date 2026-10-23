use super::*;

#[test]
fn the_tree_sample_opens_on_the_left_of_a_row_and_its_menu_from_its_button() {
    let mut test = alone(WIDE, Page::Tree);
    test.frame(Vec::new());
    let switch = test.rect_of("demo.tree.comfortable");
    test.click(pos2(switch.left() + 12.0, switch.center().y));
    test.frame(Vec::new());
    let comfortable = test.rect_of("demo.tree.1.row").height();
    assert!(
        comfortable >= 44.0,
        "comfortable rows are tall enough to tap, not {comfortable}"
    );

    let row = test.rect_of("demo.tree.1.row");
    let toggle = test.rect_of("demo.tree.1.chevron");
    assert!(
        toggle.left() < row.left() - 40.0,
        "the whole indent beside a row opens and closes it"
    );
    test.click(pos2(toggle.left() + 2.0, row.center().y));
    test.frame(Vec::new());
    assert!(
        !test.shows("demo.tree.2.row"),
        "tapping the left of a folder closes it"
    );

    test.click("demo.tree.5.more");
    test.frame(Vec::new());
    let root = test.document().root().expect("the demo has a root");
    assert!(
        showing(test.document(), root, "Rename") > 0,
        "the row's button opens its menu"
    );
    test.snapshot("tree_comfortable_with_a_row_menu");
}

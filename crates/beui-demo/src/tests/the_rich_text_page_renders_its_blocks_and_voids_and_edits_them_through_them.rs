use super::*;

const SECOND_ITEM: &str = "text.checkbox.10";

fn checked(test: &DocumentTest) -> bool {
    let checkbox = test.document().find_test_id(SECOND_ITEM).unwrap();
    test.document().api::<ToggleApi>(checkbox).checked().get()
}

#[test]
fn the_rich_text_page_renders_its_blocks_and_voids_and_edits_them_through_them() {
    let mut test = alone(WIDE, Page::RichText);
    let editor = test.rect_of("demo.rich.editor");
    let root = test.document().root().expect("the demo built a root");
    assert_eq!(
        showing(test.document(), root, "@ada"),
        1,
        "the seeded mention renders"
    );
    assert_eq!(
        showing(test.document(), root, "2."),
        1,
        "numbered items count up"
    );

    test.click(pos2(editor.left() + 200.0, editor.top() + 40.0));
    test.frame(Vec::new());
    press_key(&mut test, beui::Key::End);
    test.click("demo.rich.mention");
    test.frame(Vec::new());
    assert_eq!(
        showing(test.document(), root, "@grace"),
        1,
        "the mention button inserts a void the page renders"
    );

    test.click("demo.rich.@ada");
    test.frame(vec![Input::Text("Ada".to_owned())]);
    test.frame(Vec::new());
    assert_eq!(
        showing(test.document(), root, "@ada"),
        0,
        "a click selects the whole void, so typing replaces it"
    );

    assert!(!checked(&test));
    test.click(SECOND_ITEM);
    test.frame(Vec::new());
    assert!(
        checked(&test),
        "a checkbox the page renders checks its item"
    );
    test.snapshot("rich_text_blocks_and_voids");
}

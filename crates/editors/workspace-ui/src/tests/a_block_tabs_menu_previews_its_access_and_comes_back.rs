use super::*;

fn choose(fixture: &mut Fixture, words: &str) {
    let document = fixture.test.document();
    let root = document.root().expect("the workspace has a root");
    let row =
        text_within(&document, root, words).unwrap_or_else(|| panic!("the menu lists {words}"));
    let at = document
        .node_rect(row)
        .expect("the row is laid out")
        .center();
    drop(document);
    fixture.test.click(at);
    fixture.settle();
}

fn shows_text(fixture: &Fixture, words: &str) -> bool {
    let document = fixture.test.document();
    let root = document.root().expect("the workspace has a root");
    text_within(&document, root, words).is_some_and(|node| document.node_rect(node).is_some())
}

#[test]
fn a_block_tabs_menu_previews_its_access_and_comes_back() {
    let (mut fixture, opened) = editor_sized(Some(Vec2::new(1000.0, 600.0)));
    show(&mut fixture, opened, None);

    fixture.test.click("dock.menu");
    fixture.settle();
    choose(&mut fixture, "Access: Editing");
    choose(&mut fixture, "No access");
    assert!(
        shows_text(
            &fixture,
            "An account that only knows this block exists cannot open it."
        ),
        "choosing No access from the tab's menu shows the block as someone who cannot open it"
    );

    fixture.test.click("dock.menu");
    fixture.settle();
    choose(&mut fixture, "Access: No access");
    choose(&mut fixture, "Editing");
    assert!(
        !shows_text(
            &fixture,
            "An account that only knows this block exists cannot open it."
        ),
        "the menu stays with the tab, so the same menu brings the block back"
    );
}

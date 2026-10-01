use super::*;

fn offers_close(test: &BeuiTest<ChildApp>) -> bool {
    test.menu().iter().any(|entry| entry.id == "editor.close")
}

#[test]
fn the_menu_offers_close_only_to_a_framed_child() {
    let tab = editor().with_top_bar(false);
    assert!(!tab.menu().is_empty(), "a tab offers its menu");
    assert!(!offers_close(&tab));

    let mut framed = editor().with_top_bar(true);
    assert!(offers_close(&framed));
    framed.pick_menu("editor.close");

    assert!(
        framed.exited(),
        "close asks the host to leave the framed child"
    );
}

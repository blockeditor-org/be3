use super::*;

#[test]
fn the_top_bar_offers_close_only_to_a_framed_child() {
    let tab = editor().with_top_bar(false);
    assert!(tab.shown("editor.name"));
    assert!(!tab.shown("editor.close"));

    let mut framed = editor().with_top_bar(true);
    assert!(framed.shown("editor.close"));
    framed.click("editor.close");
    framed.run();

    assert!(
        framed.exited(),
        "close asks the host to leave the framed child"
    );
}

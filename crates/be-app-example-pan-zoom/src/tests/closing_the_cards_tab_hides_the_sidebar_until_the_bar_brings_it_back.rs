use super::*;

#[test]
fn closing_the_cards_tab_hides_the_sidebar_until_the_bar_brings_it_back() {
    let (test, _editor) = editor();
    let mut test = test.in_viewport();
    test.run();
    let docked = test.content_rect().expect("the editor reports its stage");
    assert!(test.shown("pan_zoom.title"));
    assert!(!test.shown("pan_zoom.show_sidebar"));

    assert!(
        !test.shown("dock.sidebar.close"),
        "the sidebar's lone tab carries the only close button"
    );
    test.click("dock.tab.cards.close");
    test.run();

    assert!(!test.shown("pan_zoom.title"));
    assert!(test.shown("pan_zoom.show_sidebar"));
    assert!(
        test.shown("pan_zoom.zoom_in"),
        "the zoom bar stays over the stage"
    );
    assert_eq!(test.content_rect(), Some(test.rect()));
    test.snapshot("closing_the_cards_tab_hides_the_sidebar_until_the_bar_brings_it_back");

    test.click("pan_zoom.show_sidebar");
    test.run();

    assert!(test.shown("pan_zoom.title"));
    assert!(!test.shown("pan_zoom.show_sidebar"));
    assert_eq!(test.content_rect(), Some(docked));
}

use super::*;

fn close(page: Page) -> String {
    format!("dock.tab.{}.close", page.tab().value())
}

#[test]
fn the_catalog_shows_a_page_in_place_of_the_one_shown_unless_ctrl_is_held() {
    let mut test = demo(WIDE);
    assert!(test.shows(&close(Page::Docking)));

    open(&mut test, Page::Text);
    assert!(
        test.shows(&close(Page::Text)) && !test.shows(&close(Page::Docking)),
        "a click shows the page in place of the one that was shown"
    );

    open_beside(&mut test, Page::Buttons);
    assert!(
        test.shows(&close(Page::Text)) && test.shows(&close(Page::Buttons)),
        "Ctrl and a click opens the page in a tab of its own"
    );

    open(&mut test, Page::Text);
    assert!(
        test.shows(&close(Page::Buttons)),
        "a click on a page that is open shows its tab and replaces nothing"
    );

    open(&mut test, Page::Inputs);
    assert!(
        test.shows(&close(Page::Buttons)) && !test.shows(&close(Page::Text)),
        "the next click replaces the page shown last"
    );
    assert!(
        test.rect_of(&close(Page::Inputs)).min.x < test.rect_of(&close(Page::Buttons)).min.x,
        "in the tab it had"
    );
    test.snapshot("docking_a_page_replaced_from_the_catalog");
}

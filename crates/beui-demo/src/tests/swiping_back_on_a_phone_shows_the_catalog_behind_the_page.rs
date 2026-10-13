use beui::{BackEdge, BackGesture, Event};

use super::*;

const PHONE: Vec2 = Vec2::new(390.0, 844.0);
const FRAME_LIMIT: usize = 60;

#[test]
fn swiping_back_on_a_phone_shows_the_catalog_behind_the_page() {
    let mut test = demo(PHONE);
    open(&mut test, Page::Buttons);
    assert!(!test.shows("demo.catalog.Buttons"));

    test.frame(vec![
        Event::Back(BackGesture::Started {
            edge: BackEdge::Left,
        }),
        Event::Back(BackGesture::Progressed(1.0)),
    ]);
    assert!(
        test.shows("demo.catalog.Buttons"),
        "the catalog shows behind the page while back is swiped"
    );
    test.snapshot("swiping_back_shows_the_catalog_behind_the_page");

    test.frame(vec![Event::Back(BackGesture::Invoked)]);
    for _ in 0..FRAME_LIMIT {
        if test.document().find_test_id("dock.back").is_none() {
            break;
        }
        test.frame(Vec::new());
    }
    assert!(
        test.document().find_test_id("dock.back").is_none(),
        "back reaches the catalog"
    );
    assert!(test.shows("demo.catalog.Buttons"));
}

use super::*;

#[test]
fn the_treemap_sample_goes_into_a_folder_and_back_out() {
    let mut test = alone(WIDE, Page::Tree);
    test.frame(Vec::new());
    reveal(&mut test, "demo.treemap.up");
    test.frame(Vec::new());
    let photos = test.rect_of("demo.treemap.tile.0");
    let notes = test.rect_of("demo.treemap.tile.3");
    assert!(
        photos.width() * photos.height() > notes.width() * notes.height() * 10.0,
        "a tile's area follows its size"
    );

    test.click("demo.treemap.tile.0");
    test.frame(Vec::new());
    assert!(
        test.shows("demo.treemap.tile.5"),
        "a press goes into Photos"
    );
    assert!(!test.shows("demo.treemap.tile.1"), "the top level is gone");
    assert!(
        focused(&test, "demo.treemap.tile.5"),
        "the largest tile inside takes the focus"
    );
    press_key(&mut test, beui::Key::Enter);
    assert!(test.shows("demo.treemap.tile.7"), "Enter goes into 2025");

    press_key(&mut test, beui::Key::Backspace);
    assert!(
        focused(&test, "demo.treemap.tile.5"),
        "coming back out focuses the tile gone into"
    );
    press_key(&mut test, beui::Key::ArrowRight);
    assert!(
        focused(&test, "demo.treemap.tile.6"),
        "the arrow keys move to the neighbouring tile"
    );
    test.click("demo.treemap.up");
    test.frame(Vec::new());
    assert!(
        test.shows("demo.treemap.tile.4"),
        "Up comes back to the top"
    );
    assert!(!test.shows("demo.treemap.tile.5"));
    test.snapshot("treemap_back_at_the_top");
}

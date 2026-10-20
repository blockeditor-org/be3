use super::*;
use beui::Key;

#[test]
fn the_arrow_keys_move_across_a_menu_grid_and_on_to_the_rows_below() {
    let mut test = alone(WIDE, Page::Menus);
    reveal(&mut test, "demo.slide");
    test.click("demo.slide");
    test.frame(Vec::new());
    test.snapshot("a_menu_caption_heads_a_grid_of_tiles");

    let tiles = ["demo.slide.0", "demo.slide.1", "demo.slide.2"];
    let first = test.rect_of(tiles[0]);
    assert!(
        (test.rect_of(tiles[1]).top() - first.top()).abs() < 1.0,
        "two tiles share a line"
    );
    assert!(
        test.rect_of(tiles[2]).top() > first.bottom(),
        "the third tile starts the next line"
    );

    press(&mut test, Key::ArrowDown);
    assert!(
        focused(&test, "demo.slide.0"),
        "Down enters the grid past the caption"
    );
    press(&mut test, Key::ArrowRight);
    assert!(focused(&test, "demo.slide.1"), "Right moves across");
    press(&mut test, Key::ArrowDown);
    assert!(focused(&test, "demo.slide.3"), "Down keeps the column");
    press(&mut test, Key::ArrowLeft);
    assert!(focused(&test, "demo.slide.2"), "Left moves back");
    press(&mut test, Key::ArrowDown);
    press(&mut test, Key::ArrowDown);
    assert!(
        !tiles.iter().any(|tile| focused(&test, tile))
            && !focused(&test, "demo.slide.4")
            && !focused(&test, "demo.slide.5"),
        "Down from the last line leaves the grid for the row below"
    );
    press(&mut test, Key::ArrowUp);
    assert!(
        focused(&test, "demo.slide.4"),
        "Up from below enters the grid's last line"
    );
}

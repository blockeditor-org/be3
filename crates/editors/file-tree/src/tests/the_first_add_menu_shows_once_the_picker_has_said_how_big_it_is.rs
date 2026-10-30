use super::*;

#[test]
fn the_first_add_menu_shows_once_the_picker_has_said_how_big_it_is() {
    let mut fixture = editor();
    containers_hold_blocks(&mut fixture);
    let files = my_files(&mut fixture, Vec::new());
    fixture.settle();

    fixture.test.click(format!("file-tree.{files}.add"));
    fixture.settle();
    let guessed = fixture
        .test
        .placed_picker()
        .expect("the add button places a picker");
    assert_eq!(
        guessed.opacity, 0.0,
        "the picker is placed but not shown while its size is a guess, so the menu never jumps"
    );

    fixture.test.show_picker(Vec2::new(240.0, 160.0), false);
    fixture.settle();
    let shown = fixture
        .test
        .placed_picker()
        .expect("the picker stays placed");
    assert_eq!(
        shown.opacity, 1.0,
        "it shows once it has said how big it is"
    );
    assert_eq!(shown.rect.size(), Vec2::new(240.0, 160.0));
}

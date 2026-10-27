use super::*;
use block_editor_beui::beui::PointerButton;

#[test]
fn a_trim_keeps_following_the_pointer_once_its_first_edit_lands() {
    let (mut fixture, clip) = editor_with_clip();

    let body = fixture.editor.rect_of(&format!("video.clip.{clip}"));
    let scale = body.width() / 30.0;
    let from = fixture
        .editor
        .rect_of(&format!("video.trim.{clip}"))
        .center();
    fixture.editor.down(from, PointerButton::Primary);
    fixture
        .editor
        .hover(Pos2::new(from.x + 10.0 * scale, from.y));
    assert_eq!(fixture.video().clips()[0].length, 40);

    let to = Pos2::new(from.x + 20.0 * scale, from.y);
    fixture.editor.hover(to);
    fixture.editor.up(to, PointerButton::Primary);

    assert_eq!(
        fixture.video().clips()[0].length,
        50,
        "the handle the first edit lengthened still holds the press"
    );
}

use super::*;
use be_sdk_beui::beui::Pos2;

#[test]
fn a_shape_carried_into_the_edge_band_scrolls_the_world_under_it() {
    let shape = card();
    let mut editor = editor(std::slice::from_ref(&shape));
    let item = format!("infinite-canvas.entity.{}", shape.id);
    let from = editor.rect_of(&item).center();
    let canvas = editor.rect_of("infinite-canvas.canvas");
    let edge = Pos2::new(canvas.right() - 10.0, from.y);

    editor.pen(PenPhase::Down(PointerButton::Primary), from, 0.5);
    editor.pen(PenPhase::Move, from + Vec2::new(40.0, 0.0), 0.5);
    editor.pen(PenPhase::Move, edge, 0.5);
    let reached = entities(&editor)[0].transform.center.x;
    for _ in 0..10 {
        editor.step(Vec::new());
    }
    let scrolled = entities(&editor)[0].transform.center.x;
    assert!(
        scrolled > reached + 10.0,
        "held in the edge band, the camera pans on and carries the shape further into the world: {reached} then {scrolled}"
    );
    assert!(
        (editor.rect_of(&item).center().x - edge.x).abs() < 1.0,
        "the shape stays under the still pointer"
    );

    editor.pen(PenPhase::Up(PointerButton::Primary), edge, 0.0);
    let dropped = entities(&editor)[0].transform.center.x;
    assert!(
        dropped >= scrolled,
        "it is dropped where the world had scrolled to"
    );
}

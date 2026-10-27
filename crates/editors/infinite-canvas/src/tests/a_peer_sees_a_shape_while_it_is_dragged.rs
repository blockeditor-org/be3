use super::*;
use block_editor_beui::beui::{PenPhase, PointerButton};

#[test]
fn a_peer_sees_a_shape_while_it_is_dragged() {
    let mut drawer = editor(&[]);
    let mut watcher = editor(&[]);

    drawer.click("infinite-canvas.dock.tool.Ellipse");
    let from = drawer.rect_of("infinite-canvas.canvas").center() - Vec2::new(120.0, 80.0);
    drawer.pen(PenPhase::Down(PointerButton::Primary), from, 0.5);
    drawer.next_frame();
    drawer.pen(PenPhase::Move, from + Vec2::new(200.0, 100.0), 0.5);
    drawer.run();
    sync(&mut drawer, &mut watcher);
    let [first] = &entities(&watcher)[..] else {
        panic!("the shape being dragged reaches the peer as the shape itself");
    };
    assert_eq!(first.shape, CanvasShape::Ellipse);
    assert!(
        watcher
            .document()
            .find_test_id(&format!("infinite-canvas.entity.{}", first.id))
            .is_some(),
        "the peer draws it"
    );

    drawer.next_frame();
    drawer.pen(PenPhase::Move, from + Vec2::new(240.0, 160.0), 0.5);
    drawer.run();
    sync(&mut drawer, &mut watcher);
    let [second] = &entities(&watcher)[..] else {
        panic!("the shape is still one entity");
    };
    assert_eq!(second.transform.size, CanvasPoint::new(240.0, 160.0));
    assert_eq!(
        (second.id, second.style.seed),
        (first.id, first.style.seed),
        "the shape keeps its id and its hand-drawn seed while it is dragged"
    );

    drawer.pen(
        PenPhase::Up(PointerButton::Primary),
        from + Vec2::new(240.0, 160.0),
        0.0,
    );
    drawer.run();
    sync(&mut drawer, &mut watcher);
    assert_eq!(entities(&watcher), entities(&drawer));
    assert_eq!(entities(&drawer)[0].transform, second.transform);
}

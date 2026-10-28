use super::*;
use crate::presence::CanvasCursor;
use block_editor_beui::beui::{PenPhase, PointerButton};

fn drawing(held: &BTreeMap<Uuid, Vec<u8>>) -> Option<Uuid> {
    let cursor: CanvasCursor =
        serde_json::from_slice(&held[&CanvasCursor::ID]).expect("the drawer shows its cursor");
    cursor.drawing
}

#[test]
fn a_peer_sees_a_stroke_while_it_is_drawn() {
    let mut drawer = editor(&[]);
    let mut watcher = editor(&[]);
    drawer.presence_visible(true);
    drawer.run();
    let mut held = BTreeMap::new();

    drawer.click("infinite-canvas.dock.tool.Pen");
    let canvas = drawer.rect_of("infinite-canvas.canvas");
    let from = canvas.center() - Vec2::new(200.0, 0.0);
    let at = |along: f32| from + Vec2::new(400.0 * along, 60.0 * (along * 9.0).sin());
    drawer.pen(PenPhase::Down(PointerButton::Primary), from, 0.2);
    let steps = 60;
    for step in 1..=steps {
        let along = step as f32 / steps as f32;
        drawer.next_frame();
        drawer.pen(
            PenPhase::Move,
            at(along),
            0.2 + 0.8 * (along * std::f32::consts::PI).sin(),
        );
        drawer.run();
        if step == steps / 2 {
            sync(&mut drawer, &mut watcher);
            let [half] = &entities(&watcher)[..] else {
                panic!("the stroke reaches the peer while it is drawn");
            };
            assert_eq!(half.shape, CanvasShape::Freehand);
            assert_eq!(half.points.len(), steps / 2 + 1);
            forward_presence(&mut drawer, &mut watcher, &mut held);
            assert_eq!(
                drawing(&held),
                Some(half.id),
                "the drawer tells its peers which stroke it is drawing"
            );
        }
    }
    sync(&mut drawer, &mut watcher);
    forward_presence(&mut drawer, &mut watcher, &mut held);
    watcher.snapshot("a_peer_sees_a_stroke_while_it_is_drawn");
    drawer.snapshot("a_stroke_being_drawn_keeps_a_round_end");

    drawer.pen(PenPhase::Up(PointerButton::Primary), at(1.0), 0.0);
    drawer.run();
    sync(&mut drawer, &mut watcher);
    forward_presence(&mut drawer, &mut watcher, &mut held);
    assert_eq!(drawing(&held), None, "a lifted pen draws no stroke");
    watcher.snapshot("a_peer_sees_a_finished_stroke_taper");
    let drawn = entities(&drawer);
    assert_eq!(drawn.len(), 1);
    assert_eq!(drawn[0].points.len(), steps + 1);
    assert_eq!(
        entities(&watcher),
        drawn,
        "the peer holds the stroke as drawn"
    );
}

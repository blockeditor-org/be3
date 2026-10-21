use super::*;
use crate::presence::CanvasDraft;
use block_editor_beui::be_block::presence::PresenceKind;
use block_editor_beui::beui::{PenPhase, PointerButton};

#[test]
fn a_peer_sees_a_stroke_while_it_is_drawn() {
    let mut drawer = editor(&[]);
    let mut watcher = editor(&[]);
    drawer.presence_visible(true);
    drawer.run();
    let mut held = BTreeMap::new();

    drawer.click("infinite-canvas.dock.tool.Pen");
    drawer.run();
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
        if step == steps / 2 {
            drawer.run();
            forward_presence(&mut drawer, &mut watcher, &mut held);
        }
    }
    drawer.run();
    let sent = forward_presence(&mut drawer, &mut watcher, &mut held);
    assert_eq!(
        sent, 2,
        "only the parts of the stroke that grew are sent again"
    );
    assert_eq!(
        held.keys()
            .filter(|kind| CanvasDraft::holds(**kind))
            .count(),
        2,
        "the stroke so far is in two parts"
    );
    watcher.snapshot("a_peer_sees_a_stroke_while_it_is_drawn");

    drawer.pen(PenPhase::Up(PointerButton::Primary), at(1.0), 0.0);
    drawer.run();
    forward_presence(&mut drawer, &mut watcher, &mut held);
    assert!(
        held.keys().any(|kind| CanvasDraft::holds(*kind)),
        "the stroke stays up until the peer has its entity"
    );
    let drawn = entities(&drawer);
    assert_eq!(drawn.len(), 1);
    apply(
        &mut watcher,
        InfiniteCanvasOperation::Add {
            entity: Box::new(drawn[0].clone()),
        },
    );
    watcher.run();
    watcher.snapshot("a_peer_sees_a_stroke_while_it_is_drawn_and_then_its_entity");

    drawer.click("infinite-canvas.dock.tool.Select");
    drawer.run();
    drawer.drag(
        canvas.min + Vec2::new(10.0, 10.0),
        canvas.min + Vec2::new(30.0, 30.0),
    );
    drawer.run();
    forward_presence(&mut drawer, &mut watcher, &mut held);
    assert!(
        !held.keys().any(|kind| CanvasDraft::holds(*kind)),
        "the next gesture takes the stroke down"
    );
}

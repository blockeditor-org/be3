use super::*;
use crate::presence::CanvasDraft;
use block_editor_beui::be_block::presence::PresenceKind;
use block_editor_beui::beui::{PenPhase, PointerButton};

fn drafted(held: &BTreeMap<Uuid, Vec<u8>>) -> Vec<CanvasDraft> {
    held.iter()
        .filter(|(kind, _)| CanvasDraft::holds(**kind))
        .map(|(_, value)| serde_json::from_slice(value).unwrap())
        .collect()
}

#[test]
fn a_peer_sees_a_shape_while_it_is_dragged() {
    let mut drawer = editor(&[]);
    let mut watcher = editor(&[]);
    drawer.presence_visible(true);
    drawer.run();
    let mut held = BTreeMap::new();

    drawer.click("infinite-canvas.dock.tool.Ellipse");
    drawer.run();
    let from = drawer.rect_of("infinite-canvas.canvas").center() - Vec2::new(120.0, 80.0);
    drawer.pen(PenPhase::Down(PointerButton::Primary), from, 0.5);
    drawer.next_frame();
    drawer.pen(PenPhase::Move, from + Vec2::new(200.0, 100.0), 0.5);
    drawer.run();
    forward_presence(&mut drawer, &mut watcher, &mut held);
    let [CanvasDraft::Shape(first)] = &drafted(&held)[..] else {
        panic!("a shape being dragged is one draft");
    };
    assert_eq!(first.shape, CanvasShape::Ellipse);

    drawer.next_frame();
    drawer.pen(PenPhase::Move, from + Vec2::new(240.0, 160.0), 0.5);
    drawer.run();
    forward_presence(&mut drawer, &mut watcher, &mut held);
    let [CanvasDraft::Shape(second)] = &drafted(&held)[..] else {
        panic!("a shape being dragged is one draft");
    };
    assert_eq!(second.transform.size, CanvasPoint::new(240.0, 160.0));
    assert_eq!(
        (second.id, second.style.seed),
        (first.id, first.style.seed),
        "the draft keeps its id and its hand-drawn seed while it is dragged"
    );

    drawer.pen(
        PenPhase::Up(PointerButton::Primary),
        from + Vec2::new(240.0, 160.0),
        0.0,
    );
    drawer.run();
    let drawn = entities(&drawer);
    assert_eq!(drawn.len(), 1);
    assert_eq!(
        (drawn[0].id, drawn[0].style.seed, drawn[0].transform),
        (second.id, second.style.seed, second.transform),
        "the shape lands as the draft the peer saw"
    );
}

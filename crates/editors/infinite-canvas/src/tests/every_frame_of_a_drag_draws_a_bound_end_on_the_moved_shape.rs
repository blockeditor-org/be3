use super::*;
use block_editor_beui::beui::{Input, Pos2, Rect};
use infinite_canvas_types::CanvasBinding;

use crate::app::canvas::item_bounds;

struct Placing {
    origin: Pos2,
    scale: f32,
}

impl Placing {
    fn world(&self, at: Pos2) -> CanvasPoint {
        CanvasPoint::new(
            (at.x - self.origin.x) / self.scale,
            (at.y - self.origin.y) / self.scale,
        )
    }

    fn screen(&self, world: Rect) -> Rect {
        Rect::from_min_max(
            self.origin + world.min.to_vec2() * self.scale,
            self.origin + world.max.to_vec2() * self.scale,
        )
    }
}

fn shown(editor: &BeuiTest<CanvasApp>, id: Uuid) -> Rect {
    editor.rect_of(&format!("infinite-canvas.entity.{id}"))
}

fn assert_follows(
    editor: &BeuiTest<CanvasApp>,
    placing: &Placing,
    target: &CanvasEntity,
    arrow: &CanvasEntity,
    frame: &str,
) {
    let mut moved = target.clone();
    moved.transform.center = placing.world(shown(editor, target.id).center());
    let expected = placing.screen(item_bounds(&arrow.followed(|_| Some(&moved))));
    let drawn = shown(editor, arrow.id);
    assert!(
        (drawn.min - expected.min).length() < 1.0 && (drawn.max - expected.max).length() < 1.0,
        "{frame}: the arrow is drawn at {drawn:?}, not ending on the shape drawn in the same frame ({expected:?})"
    );
}

#[test]
fn every_frame_of_a_drag_draws_a_bound_end_on_the_moved_shape() {
    let mut target = card();
    target.transform = CanvasTransform::new(
        CanvasPoint::new(150.0, 0.0),
        CanvasPoint::new(100.0, 80.0),
        0.0,
    );
    let mut arrow = path(
        Uuid::from_u128(3),
        &[CanvasPoint::new(-150.0, 0.0), CanvasPoint::new(96.0, 0.0)],
    );
    arrow.style.end_arrowhead = CanvasArrowhead::Arrow;
    arrow.bindings[1] = Some(CanvasBinding {
        entity: target.id,
        anchor: CanvasPoint::default(),
    });
    let canvas = vector([target.clone(), arrow.clone()]);
    let mut editor = open(&canvas, false, &[]);
    let mut peer = open(&canvas, false, &[]);
    let at_rest = shown(&editor, target.id);
    let scale = at_rest.width() / item_bounds(&target).width();
    let placing = Placing {
        origin: at_rest.center() - Vec2::new(150.0, 0.0) * scale,
        scale,
    };

    let grab = at_rest.center();
    editor.down(grab, PointerButton::Primary);
    editor.run();
    sync(&mut editor, &mut peer);
    for step in 1..=12 {
        let at = grab + Vec2::new(step as f32 * 7.0, step as f32 * 11.0);
        editor.step(vec![Input::PointerMoved(at)]);
        assert_follows(
            &editor,
            &placing,
            &target,
            &arrow,
            &format!("frame {step} of the drag"),
        );
        for operation in editor.store().take_written(None) {
            peer.store().apply_elsewhere(None, operation);
        }
        peer.step(Vec::new());
        assert_follows(
            &peer,
            &placing,
            &target,
            &arrow,
            &format!("the peer's frame {step}"),
        );
    }
    editor.up(grab + Vec2::new(84.0, 132.0), PointerButton::Primary);
    editor.run();
    sync(&mut editor, &mut peer);
    assert_follows(&editor, &placing, &target, &arrow, "after the drag");
    assert_follows(&peer, &placing, &target, &arrow, "the peer after the drag");
    assert!(
        shown(&editor, target.id).center().y > grab.y + 100.0,
        "the drag moved the shape"
    );
}

use super::*;
use be_sdk_beui::beui::{Input, Rect};
use be_type_infinite_canvas::CanvasBinding;

fn shown(editor: &BeuiTest<CanvasRoot>, id: Uuid) -> Rect {
    editor.rect_of(&format!("infinite-canvas.entity.{id}"))
}

fn assert_same(mover: &BeuiTest<CanvasRoot>, watcher: &BeuiTest<CanvasRoot>, id: Uuid, at: &str) {
    let (here, there) = (shown(mover, id), shown(watcher, id));
    assert!(
        (here.min - there.min).length() < 1.0 && (here.max - there.max).length() < 1.0,
        "{at}: the peer draws {id} at {there:?}, the mover at {here:?}"
    );
}

/// One device resizes a shape a line is bound to: the other, on its own device, previews the
/// held resize from its replica, drawing the shape and the line that follows it at every frame
/// before the release, though nothing is written, and the same once the resize commits.
#[test]
fn a_peer_draws_a_bound_line_following_each_frame_of_a_held_resize() {
    let mut target = card();
    target.style.stroke_width = 0.0;
    let mut arrow = path(
        Uuid::from_u128(3),
        &[CanvasPoint::new(-300.0, 0.0), CanvasPoint::new(-96.0, 0.0)],
    );
    arrow.style.end_arrowhead = CanvasArrowhead::Arrow;
    arrow.bindings[1] = Some(CanvasBinding {
        entity: target.id,
        anchor: CanvasPoint::default(),
    });
    let canvas = vector([target.clone(), arrow.clone()]);
    let mut mover = open(&canvas, false, &[]);
    let mut watcher = open(&canvas, false, &[]);
    join_session(&mut mover, &mut watcher);
    let written = entities(&watcher);
    let mut submitted = 0;

    let shape = format!("infinite-canvas.entity.{}", target.id);
    let before = mover.rect_of(&shape);
    let line_before = shown(&mover, arrow.id);
    let scale = before.width() / 184.0;
    mover.click(&shape);
    relay_session(&mut mover, &mut watcher, FIRST_PEER);
    let corner = before.center() + Vec2::new(90.0, 60.0) * scale;
    let from = corner + Vec2::new(-4.0, -4.0);
    mover.down(from, PointerButton::Primary);
    relay_session(&mut mover, &mut watcher, FIRST_PEER);
    for step in 1..=6 {
        let at = from + Vec2::new(step as f32 * 8.0, step as f32 * 12.0);
        mover.step(vec![Input::PointerMoved(at)]);
        submitted += relay_session(&mut mover, &mut watcher, FIRST_PEER);
        let frame = format!("frame {step} of the resize");
        assert_same(&mover, &watcher, target.id, &frame);
        assert_same(&mover, &watcher, arrow.id, &frame);
        assert_eq!(entities(&watcher), entities(&mover), "{frame}");
        if step == 6 {
            watcher.snapshot("a_peer_draws_a_bound_line_following_each_frame_of_a_held_resize");
        }
    }
    assert_eq!(submitted, 0, "nothing of the held resize is sent to the order");
    assert!(
        shown(&watcher, target.id).height() > before.height() + 40.0,
        "the peer shows the shape resized"
    );
    assert_ne!(
        shown(&watcher, arrow.id),
        line_before,
        "the peer's line follows the resized shape"
    );

    mover.up(from + Vec2::new(48.0, 72.0), PointerButton::Primary);
    assert!(relay_session(&mut mover, &mut watcher, FIRST_PEER) > 0, "the release sends the resize");
    assert_eq!(entities(&watcher), entities(&mover), "the resize is written");
    assert_ne!(entities(&watcher), written);
    assert_same(&mover, &watcher, target.id, "after the release");
    assert_same(&mover, &watcher, arrow.id, "after the release");
    watcher.snapshot("a_peer_draws_a_bound_line_on_a_resized_shape_once_the_resize_commits");
}

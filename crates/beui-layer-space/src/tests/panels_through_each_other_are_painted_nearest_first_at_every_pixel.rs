use std::rc::Rc;

use beui_core::color::Color32;
use beui_core::context::WindowOutput;
use beui_core::damage::Region;
use beui_core::window::WindowState;
use glam::Quat;

use super::*;

fn solid(id: WindowId, size: Vec2, color: Color32) -> WindowOutput {
    let mut state = WindowState::default();
    state.appearance.clear_color = color;
    WindowOutput {
        id,
        size,
        layers: Rc::new(Vec::new()),
        window: state,
        changed: true,
        damage: Region::NOTHING,
        moved: None,
        pixels_per_point: 1.0,
    }
}

fn opened(space: &mut Space, now: Instant, position: Vec3, rotation: Quat, bend: f32) -> WindowId {
    let mut placed = panel(WindowId::MAIN, Pose { position, rotation });
    placed.metres_per_point = 0.003;
    placed.bend = bend;
    open(space, now, WindowPlacement::Host(placed.placement())).0
}

#[test]
fn panels_through_each_other_are_painted_nearest_first_at_every_pixel() {
    let (mut space, now) = space();
    let (left, right) = (
        main(&space).world(pos2(250.0, 300.0)),
        main(&space).world(pos2(560.0, 330.0)),
    );
    let crossing = opened(&mut space, now, left, Quat::from_rotation_y(0.9), 0.0);
    let curved = opened(&mut space, now, right, Quat::from_rotation_x(-0.6), 1.2);
    let colours = [
        (WindowId::MAIN, Color32::from_rgb(200, 204, 212)),
        (crossing, Color32::from_rgb(220, 90, 80)),
        (curved, Color32::from_rgb(70, 120, 220)),
    ];
    let windows = [
        solid(WindowId::MAIN, PLATFORM, colours[0].1),
        solid(crossing, SMALL, colours[1].1),
        solid(curved, SMALL, colours[2].1),
    ];
    let frame = space.scene(&windows, &view(now));
    let painting = crate::picture::painting(&frame, Rect::from_min_size(Pos2::ZERO, PLATFORM))
        .expect("the scene paints");
    let pixels =
        beui_core::painting::rasterize(&painting.frame, painting.frame.background, |key| {
            painting.textures.get(&key)
        })
        .expect("the painting rasterises");
    let snapshot =
        paint_snapshot::Snapshot::of_painting(painting.clone()).expect("the painting encodes");
    paint_snapshot::assert_accepted(
        "panels_through_each_other_are_painted_nearest_first_at_every_pixel",
        &snapshot,
        "./scripts/test panels_through_each_other_are_painted_nearest_first_at_every_pixel --update",
    );

    let mut wrong = Vec::new();
    let mut seen = std::collections::BTreeSet::new();
    for y in (4..PLATFORM.y as u32).step_by(6) {
        for x in (4..PLATFORM.x as u32).step_by(6) {
            let ray = space
                .head
                .ray(PLATFORM, vec2(x as f32 + 0.5, y as f32 + 0.5));
            let mut hits = Vec::new();
            let mut unsure = false;
            for panel in &space.panels {
                let Some(hit) = panel.hit(ray) else {
                    continue;
                };
                let margin = panel.size * 0.06;
                let inside = (margin.x..panel.size.x - margin.x).contains(&hit.pos.x)
                    && (margin.y..panel.size.y - margin.y).contains(&hit.pos.y);
                let normal = panel.frame_at(hit.pos).1 * Vec3::Z;
                unsure |= !inside || normal.dot(-ray.direction) < 0.3;
                hits.push((hit.distance, panel.window));
            }
            if unsure {
                continue;
            }
            hits.sort_by(|a, b| a.0.total_cmp(&b.0));
            let [(nearest, front), (next, back), ..] = hits[..] else {
                continue;
            };
            let chrome = chrome::pick(&space.panels, ray, None)
                .is_some_and(|(_, hit)| hit.distance < next + 0.01);
            if next - nearest < 0.01 || chrome {
                continue;
            }
            seen.insert((front, back));
            let expected = colours
                .iter()
                .find(|(window, _)| *window == front)
                .unwrap()
                .1
                .to_array();
            let got = pixels.rgba[(y * PLATFORM.x as u32 + x) as usize];
            if got[..3] != expected[..3] {
                wrong.push((x, y, got, expected));
            }
        }
    }
    for (front, back) in [
        (crossing, WindowId::MAIN),
        (WindowId::MAIN, crossing),
        (curved, WindowId::MAIN),
        (WindowId::MAIN, curved),
    ] {
        assert!(
            seen.contains(&(front, back)),
            "some pixels show {front:?} in front of {back:?}: {seen:?}"
        );
    }
    assert!(
        wrong.is_empty(),
        "each pixel shows the panel nearest there: {} wrong, first {:?}",
        wrong.len(),
        wrong.first()
    );
}

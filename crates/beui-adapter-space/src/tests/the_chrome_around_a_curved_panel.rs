use std::rc::Rc;

use beui_core::app::{Setup, Waker};
use beui_core::color::Color32;
use beui_core::context::{Context, WindowOutput};
use beui_core::damage::Region;
use beui_core::display::Layer;
use beui_core::font::{FontId, TextLayout};
use beui_core::geometry::Rotation;
use beui_core::icons::{ICON_FOLDER, ICON_SEARCH, ICON_SETTINGS};
use beui_core::painter::{Corners, Shape};
use beui_core::window::WindowState;
use beui_font_freetype::FreetypeFonts;
use glam::Quat;

use super::*;

fn rect(rect: Rect, radius: f32, color: Color32) -> Layer {
    Layer::Shape(Shape::Rect {
        rect,
        corner_radius: Corners::all(radius),
        stroke_width: 0.0,
        color,
        rotation: Rotation::NONE,
        clip: Rect::EVERYTHING,
    })
}

fn text(context: &Context, at: Pos2, text: &str, font: FontId, color: Color32) -> Layer {
    Layer::Shape(Shape::Text {
        origin: at,
        galley: context.layout(text, font, TextLayout::DEFAULT),
        scale: 1.0,
        color,
        rotation: Rotation::NONE,
        clip: Rect::EVERYTHING,
    })
}

pub(super) fn window(context: &Context, id: WindowId, size: Vec2, title: &str) -> WindowOutput {
    let ink = Color32::from_rgb(40, 44, 52);
    let mut layers = vec![
        rect(
            Rect::from_min_size(Pos2::ZERO, vec2(size.x, 44.0)),
            0.0,
            Color32::from_rgb(232, 236, 244),
        ),
        text(
            context,
            pos2(16.0, 10.0),
            title,
            FontId::proportional(18.0),
            ink,
        ),
    ];
    for (index, icon) in [ICON_FOLDER, ICON_SEARCH, ICON_SETTINGS]
        .into_iter()
        .enumerate()
    {
        let x = size.x - 36.0 * (index as f32 + 1.0);
        layers.push(text(
            context,
            pos2(x, 10.0),
            icon.as_str(),
            FontId::icons(20.0),
            ink,
        ));
    }
    let columns = ((size.x - 16.0) / 136.0).floor().max(1.0) as usize;
    let rows = ((size.y - 60.0) / 96.0).floor().max(1.0) as usize;
    for row in 0..rows {
        for column in 0..columns {
            let at = pos2(16.0 + column as f32 * 136.0, 60.0 + row as f32 * 96.0);
            let hue = ((row * columns + column) * 37 % 120) as u8;
            layers.push(rect(
                Rect::from_min_size(at, vec2(120.0, 80.0)),
                10.0,
                Color32::from_rgb(90 + hue, 140, 220 - hue),
            ));
        }
    }
    let mut state = WindowState::default();
    state.appearance.clear_color = Color32::from_rgb(248, 249, 252);
    WindowOutput {
        id,
        size,
        layers: Rc::new(layers),
        window: state,
        changed: true,
        damage: Region::NOTHING,
        moved: None,
        pixels_per_point: 1.0,
    }
}

#[test]
fn the_chrome_around_a_curved_panel() {
    let (mut space, now) = space();
    let context = Context::new(FreetypeFonts::default());
    let mut setup = Setup::new(Waker::new(|| {}));
    setup.provide(context.clone());
    space.setup(&setup);
    space.head.position.z += 0.45;
    space.panels[0].bend = 1.2;

    let side = Pose {
        position: main(&space).world(pos2(PLATFORM.x, 0.0)) + Vec3::new(0.18, -0.2, 0.15),
        rotation: Quat::from_rotation_y(-0.5),
    };
    let other = open(
        &mut space,
        now,
        WindowPlacement::Host(panel(WindowId::MAIN, side).placement()),
    )
    .0;

    let bend = on_part(&space, WindowId::MAIN, Part::Bend);
    send(
        &mut space,
        now,
        [
            Input::PointerMoved(bend),
            button(bend, true),
            button(bend, false),
        ],
    );
    let turn = on_part(&space, WindowId::MAIN, Part::Turn);
    send(&mut space, now, [Input::PointerMoved(turn)]);
    assert_eq!(space.chrome.hovered, Some((WindowId::MAIN, Part::Turn)));

    let windows = [
        window(&context, WindowId::MAIN, PLATFORM, "Curved"),
        window(&context, other, SMALL, "Flat"),
    ];
    let frame = space.scene(&windows, &view(now));
    let painting = crate::picture::painting(&frame, Rect::from_min_size(Pos2::ZERO, PLATFORM))
        .expect("the scene paints");
    let snapshot = paint_snapshot::Snapshot::of_painting(painting).expect("the painting encodes");
    paint_snapshot::assert_accepted(
        "the_chrome_around_a_curved_panel",
        &snapshot,
        "./scripts/test the_chrome_around_a_curved_panel --update",
    );
}

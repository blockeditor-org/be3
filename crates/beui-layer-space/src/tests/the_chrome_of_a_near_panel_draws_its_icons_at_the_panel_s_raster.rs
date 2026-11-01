use beui_core::app::{Setup, Waker};
use beui_core::context::Context;
use beui_core::display::Layer;
use beui_core::painter::Shape;
use beui_core::painting::{self, Punches};
use beui_font_freetype::FreetypeFonts;
use beui_types::Color32;

use super::the_chrome_around_a_curved_panel::window;
use super::*;

#[test]
fn the_chrome_of_a_near_panel_draws_its_icons_at_the_panel_s_raster() {
    let (mut space, now) = space();
    let context = Context::new(FreetypeFonts::default());
    let mut setup = Setup::new(Waker::new(|| {}));
    setup.provide(context.clone());
    space.setup(&setup);
    let near = open_ahead(&mut space, now, 0.3);
    let scale = space.scale(near).expect("the near panel has a scale");
    assert!(scale > 2.0, "a panel 30 cm away is drawn at {scale}");

    let windows = [
        window(&context, WindowId::MAIN, PLATFORM, "Main"),
        window(&context, near, SMALL, "Near"),
    ];
    let frame = space.scene(&windows, &view(now));
    let chrome = frame
        .chrome
        .iter()
        .find(|output| output.id == near)
        .expect("the near panel has chrome");
    let panel = space.panel(near).expect("the near panel is open");
    assert_eq!(
        chrome.pixels_per_point,
        scale / (panel.metres_per_point * crate::panel::POINTS_PER_METRE),
        "the chrome is drawn at as many pixels per metre as its window"
    );
    let icons: Vec<f32> = chrome
        .layers
        .iter()
        .filter_map(|layer| match layer {
            Layer::Shape(Shape::Text { galley, .. }) => Some(galley.pixels_per_point()),
            _ => None,
        })
        .collect();
    assert_eq!(icons.len(), 4, "the pill shows four icons");
    assert!(
        icons
            .iter()
            .all(|raster| *raster == chrome.pixels_per_point),
        "each icon is rasterised at the chrome's pixels per point, not stretched: {icons:?}"
    );

    let layout = chrome::layout(
        vec2(panel.metres().x, panel.metres().y),
        panel.chrome,
        false,
    );
    let pill = layout
        .pill
        .translate(-layout.bounds.min.to_vec2())
        .expand(0.004);
    let painting = painting::painting(
        chrome,
        chrome.size,
        chrome.pixels_per_point,
        Color32::TRANSPARENT,
        Punches::Shown,
    )
    .expect("the chrome paints")
    .cropped(
        [pill.min.x, pill.min.y, pill.max.x, pill.max.y]
            .map(|metres| metres * crate::panel::POINTS_PER_METRE),
    );
    paint_snapshot::assert_painting(
        "the_chrome_of_a_near_panel_draws_its_icons_at_the_panel_s_raster",
        painting,
    );
}

use super::*;
use crate::primitives::{Text, Window, WindowHandle};
use crate::{WindowId, WindowReport};

fn text_scales(shapes: Vec<Shape>) -> Vec<f32> {
    shapes
        .into_iter()
        .filter_map(|shape| match shape {
            Shape::Text { galley, .. } => Some(galley.pixels_per_point()),
            _ => None,
        })
        .collect()
}

#[test]
fn a_window_lays_out_and_paints_at_the_pixels_per_point_its_host_reports() {
    let _root = crate::reactive::RootScope::new();
    let window = WindowId::next();
    let document = build_node(move || {
        view! {
            <Window id={window} size={vec2(120.0, 80.0)}>|_: WindowHandle|
                <Text color=Color32::WHITE>"Panel"</Text>
            </Window>
        }
    });
    let mut harness = Harness::hosting_windows(document, VIEWPORT);
    harness.frame(Vec::new());
    let output = harness.frame(Vec::new());
    let shown = output.window(window).expect("the window is shown");
    assert_eq!(shown.pixels_per_point, 1.0);
    assert_eq!(text_scales(shown.shapes()), [1.0]);

    harness.report_window(WindowReport::Rastered {
        window,
        pixels_per_point: 2.5,
    });
    let output = harness.frame(Vec::new());
    let shown = output.window(window).expect("the window is shown");
    assert_eq!(
        shown.pixels_per_point, 2.5,
        "the window's output says the pixels per point it was drawn at"
    );
    assert_eq!(
        text_scales(shown.shapes()),
        [2.5],
        "its text is rasterised for that many pixels per point, so it is sharp at that size"
    );
    assert!(
        shown.changed && shown.damage.bounds() == Rect::from_min_size(Pos2::ZERO, shown.size),
        "the whole window is drawn again at its new scale"
    );
    assert_eq!(
        output.main().pixels_per_point,
        1.0,
        "the main window keeps its own scale"
    );

    let output = harness.frame(Vec::new());
    assert!(
        !output.window(window).is_some_and(|shown| shown.changed),
        "and once drawn at it, the window is not drawn again"
    );
}

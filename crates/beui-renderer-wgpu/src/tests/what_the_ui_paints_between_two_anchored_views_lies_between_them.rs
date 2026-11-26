use super::*;
use be_lib_layers::Surface;

#[test]
fn what_the_ui_paints_between_two_anchored_views_lies_between_them() {
    beui_draw::compose::composite_windows();
    let mut target = Target::new();
    target.renderer.set_window(true);
    let anchored = |instance| Surface::Anchored {
        instance,
        surface: 0,
    };
    let red = [255, 0, 0, 255];
    let blue = [0, 0, 255, 255];
    let first = beui_draw::drawing(Shows::new(texture_of(&target, 1, &[red]), anchored(1), 1));
    let second = beui_draw::drawing(Shows::new(texture_of(&target, 1, &[blue]), anchored(2), 2));
    let rect = |x: f32, y: f32, width: f32, height: f32| {
        Rect::from_min_max(pos2(x, y), pos2(x + width, y + height))
    };
    target.draw(Color32::BLACK, Repaint::Everything, |painter| {
        painter.rect_filled(everything(), 0.0, Color32::WHITE);
        painter.drawing(rect(8.0, 8.0, 24.0, 24.0), &first);
        // A scrim for the second view, over the first and under the second.
        painter.rect_filled(
            rect(0.0, 0.0, 64.0, 64.0),
            0.0,
            Color32::from_rgba_unmultiplied(0, 0, 0, 128),
        );
        painter.drawing(rect(24.0, 24.0, 24.0, 24.0), &second);
    });
    let surfaces: Vec<Surface> = target
        .renderer
        .layers()
        .expect("the window is composited")
        .layers
        .iter()
        .map(|layer| layer.surface)
        .collect();
    assert_eq!(
        surfaces,
        vec![
            Surface::Root(0),
            anchored(1),
            Surface::Root(1),
            anchored(2),
            Surface::Root(2),
        ],
        "each anchored view has a root of its own above it"
    );
    let capture = target.read();
    assert_eq!(
        capture.pixel(36, 36),
        blue,
        "the scrim painted before the second view lies under it"
    );
    let [dimmed, _, _, _] = capture.pixel(12, 12);
    assert!(
        dimmed > 40 && dimmed < 220,
        "and over the first: {:?}",
        capture.pixel(12, 12)
    );
}

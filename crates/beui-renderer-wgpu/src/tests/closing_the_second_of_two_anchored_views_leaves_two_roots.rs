use super::*;
use be_lib_layers::Surface;

#[test]
fn closing_the_second_of_two_anchored_views_leaves_two_roots() {
    beui_draw::compose::composite_windows();
    let mut target = Target::new();
    target.renderer.set_window(true);
    let anchored = |instance| Surface::Anchored {
        instance,
        surface: 0,
    };
    let red = [255, 0, 0, 255];
    let blue = [0, 0, 255, 255];
    let first = beui_draw::drawing(Shows::new(texture_of(&target, 1, &[red]), anchored(1), 11));
    let second = beui_draw::drawing(Shows::new(texture_of(&target, 1, &[blue]), anchored(2), 12));
    let rect = |x: f32, y: f32, size: f32| Rect::from_min_max(pos2(x, y), pos2(x + size, y + size));
    let (first, second) = (&first, &second);
    let paint = |open: bool| {
        move |painter: &Painter| {
            painter.rect_filled(everything(), 0.0, Color32::WHITE);
            painter.drawing(rect(4.0, 4.0, 16.0), first);
            if open {
                painter.drawing(rect(24.0, 24.0, 16.0), second);
            }
            painter.rect_filled(rect(50.0, 50.0, 6.0), 0.0, Color32::from_rgb(255, 255, 0));
        }
    };
    let surfaces = |target: &Target| -> Vec<Surface> {
        target
            .renderer
            .layers()
            .expect("the window is composited")
            .layers
            .iter()
            .map(|layer| layer.surface)
            .collect()
    };
    target.draw(Color32::BLACK, Repaint::Everything, paint(true));
    assert_eq!(
        surfaces(&target),
        vec![
            Surface::Root(0),
            anchored(1),
            Surface::Root(1),
            anchored(2),
            Surface::Root(2),
        ]
    );
    assert_eq!(target.read().pixel(28, 28), blue);

    // The second view closes: the UI damages where it was.
    target.draw(
        Color32::BLACK,
        Repaint::Region {
            region: Region::from(rect(24.0, 24.0, 16.0)),
            background: Color32::BLACK,
        },
        paint(false),
    );
    assert_eq!(
        surfaces(&target),
        vec![Surface::Root(0), anchored(1), Surface::Root(1)],
        "three roots become two"
    );
    let composition = target.renderer.composition.as_ref().expect("composited");
    assert_eq!(composition.roots.len(), 2, "the third root is dropped");
    let capture = target.read();
    assert_eq!(
        capture.pixel(28, 28),
        [255; 4],
        "where the view was, the UI beneath it shows"
    );
    assert_eq!(capture.pixel(8, 8), red, "the first view still shows");
    assert_eq!(
        capture.pixel(52, 52),
        [255, 255, 0, 255],
        "and what the UI painted after the views, now in the second root"
    );
}

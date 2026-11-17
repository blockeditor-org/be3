use super::*;
use be_lib_layers::Surface;

#[test]
fn changing_what_lies_between_two_anchored_views_draws_again_only_there() {
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
    let paint = |between: Color32| {
        move |painter: &Painter| {
            painter.rect_filled(everything(), 0.0, Color32::WHITE);
            painter.drawing(rect(4.0, 4.0, 16.0), first);
            painter.rect_filled(rect(40.0, 4.0, 8.0), 0.0, between);
            painter.drawing(rect(24.0, 24.0, 16.0), second);
            painter.rect_filled(rect(50.0, 50.0, 6.0), 0.0, Color32::from_rgb(255, 255, 0));
        }
    };
    target.draw(
        Color32::BLACK,
        Repaint::Everything,
        paint(Color32::from_rgb(0, 255, 0)),
    );
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
    let list = vec![
        Surface::Root(0),
        anchored(1),
        Surface::Root(1),
        anchored(2),
        Surface::Root(2),
    ];
    assert_eq!(surfaces(&target), list);

    // What the UI paints between the two views changes: the window is drawn again only there,
    // the roots above the first view are cleared there first, and the root above the second
    // keeps what it holds elsewhere.
    let magenta = Color32::from_rgb(255, 0, 255);
    target.draw(
        Color32::BLACK,
        Repaint::Region {
            region: Region::from(rect(40.0, 4.0, 8.0)),
            background: Color32::BLACK,
        },
        paint(magenta),
    );
    assert_eq!(surfaces(&target), list, "the layers stay as they were");
    let scissors = target
        .renderer
        .scissors()
        .expect("the window is drawn again in part")
        .to_vec();
    assert!(
        scissors
            .iter()
            .all(|[x, y, width, height]| *x >= 40 && *y >= 4 && x + width <= 48 && y + height <= 12),
        "only where the UI changed is drawn again: {scissors:?}"
    );
    let capture = target.read();
    assert_eq!(capture.pixel(43, 7), [255, 0, 255, 255], "the change shows");
    assert_eq!(capture.pixel(8, 8), red, "the first view still shows");
    assert_eq!(capture.pixel(28, 28), blue, "and the second");
    assert_eq!(
        capture.pixel(52, 52),
        [255, 255, 0, 255],
        "and what the UI paints over it"
    );
    let composition = target.renderer.composition.as_ref().expect("composited");
    assert_eq!(composition.roots.len(), 3, "a root above each view");
    let middle = target.read_texture(&composition.roots[1].texture);
    assert_eq!(
        middle.pixel(43, 7),
        [255, 0, 255, 255],
        "the change is painted in the root between the views"
    );
    let top = target.read_texture(&composition.roots[2].texture);
    assert_eq!(
        top.pixel(43, 7)[3],
        0,
        "the root above the second view is cleared where it was drawn again"
    );
    assert_eq!(
        top.pixel(52, 52),
        [255, 255, 0, 255],
        "and keeps what it held elsewhere"
    );
}

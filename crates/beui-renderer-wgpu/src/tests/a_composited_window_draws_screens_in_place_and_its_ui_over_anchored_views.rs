use super::*;
use be_lib_layers::Surface;

#[test]
fn a_composited_window_draws_screens_in_place_and_its_ui_over_anchored_views() {
    beui_draw::compose::composite_windows();
    let mut target = Target::new();
    target.renderer.set_window(true);
    let red = [255, 0, 0, 255];
    let yellow = [255, 255, 0, 255];
    let shows = |texture, surface: Surface| {
        beui_draw::drawing(Shows::new(
            texture,
            surface,
            10 + surface.instance().map_or(0, u64::from),
        ))
    };
    let screen = shows(
        texture_of(&target, 2, &[red; 4]),
        Surface::Screen {
            instance: 1,
            surface: 0,
        },
    );
    let anchored = shows(
        texture_of(&target, 2, &[yellow; 4]),
        Surface::Anchored {
            instance: 2,
            surface: 0,
        },
    );
    let rect = |x: f32, y: f32, size: f32| Rect::from_min_max(pos2(x, y), pos2(x + size, y + size));
    let blue = Color32::from_rgb(0, 0, 255);
    let (screen, anchored) = (&screen, &anchored);
    let paint = |chrome: Rect| {
        move |painter: &Painter| {
            painter.rect_filled(rect(0.0, 0.0, 64.0), 0.0, Color32::WHITE);
            painter.drawing(rect(8.0, 8.0, 16.0), screen);
            painter.rect_filled(rect(16.0, 16.0, 4.0), 0.0, Color32::from_rgb(0, 255, 0));
            painter.drawing(rect(32.0, 8.0, 16.0), anchored);
            painter.rect_filled(chrome, 0.0, blue);
        }
    };
    target.draw(Color32::BLACK, Repaint::Everything, paint(rect(40.0, 16.0, 4.0)));
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
            Surface::Screen {
                instance: 1,
                surface: 0
            },
            Surface::Root(0),
            Surface::Anchored {
                instance: 2,
                surface: 0
            },
            Surface::Root(1),
        ],
        "the screen lies under the host's surface, the anchored view over it, and what the host \
         paints after the anchored view over that"
    );
    let check = |capture: &Capture, chrome: (u32, u32)| {
        assert_eq!(capture.pixel(10, 10), red, "the screen shows in place");
        assert_eq!(
            capture.pixel(17, 17),
            [0, 255, 0, 255],
            "what the host paints over the screen shows over it"
        );
        assert_eq!(capture.pixel(34, 10), yellow, "the anchored view lies over the host");
        assert_eq!(
            capture.pixel(chrome.0, chrome.1),
            [0, 0, 255, 255],
            "what the host paints after the anchored view shows over it"
        );
        assert_eq!(capture.pixel(2, 2), [255; 4], "the host's own surface shows");
    };
    check(&target.read(), (41, 17));

    // The host's chrome moves within the anchored view: only where it was and is is drawn
    // again, and the layers keep showing.
    let damage = Region::from(rect(40.0, 16.0, 4.0)).union(rect(36.0, 16.0, 4.0).into());
    target.draw(
        Color32::BLACK,
        Repaint::Region {
            region: damage,
            background: Color32::BLACK,
        },
        paint(rect(36.0, 16.0, 4.0)),
    );
    let scissors = target
        .renderer
        .scissors()
        .expect("the window is drawn again in part")
        .to_vec();
    let drawn = scissors.iter().fold(None, |drawn: Option<[u32; 4]>, [x, y, width, height]| {
        let [left, top, right, bottom] = drawn.unwrap_or([*x, *y, x + width, y + height]);
        Some([left.min(*x), top.min(*y), right.max(x + width), bottom.max(y + height)])
    });
    assert_eq!(
        drawn,
        Some([36, 16, 44, 20]),
        "the window is drawn again only where its own surface changed: {scissors:?}"
    );
    let capture = target.read();
    check(&capture, (37, 17));
    assert_eq!(capture.pixel(41, 17), yellow, "where the chrome left, the anchored view shows");
}

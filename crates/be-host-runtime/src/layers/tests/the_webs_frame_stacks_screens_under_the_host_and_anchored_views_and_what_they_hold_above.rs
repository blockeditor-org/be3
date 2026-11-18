use super::*;

#[test]
fn the_webs_frame_stacks_screens_under_the_host_and_anchored_views_and_what_they_hold_above() {
    let shared = Rc::new(RefCell::new(Shared::default()));
    shared
        .borrow_mut()
        .paint(&[placement(1), placement(2), placement(3)]);
    shared.borrow_mut().publish(
        &ScreenLayout {
            screens: vec![placement(1), placement(2), placement(3)],
        },
        Vec::new(),
        None,
    );
    let editor = blit(
        &shared,
        1,
        Rect::from_min_size(Pos2::ZERO, vec2(200.0, 100.0)),
    );
    let popover = Blit {
        layer: Some(AnchoredSource {
            surface: 9,
            pixels: Rect::from_min_size(Pos2::ZERO, vec2(60.0, 60.0)),
            bounds: Rect::from_min_size(Pos2::ZERO, vec2(400.0, 70.0)),
        }),
        ..blit(
            &shared,
            9,
            Rect::from_min_size(pos2(20.0, 20.0), vec2(60.0, 60.0)),
        )
    };
    let child = blit(
        &shared,
        2,
        Rect::from_min_size(pos2(30.0, 30.0), vec2(20.0, 10.0)),
    );
    let beside = blit(
        &shared,
        3,
        Rect::from_min_size(pos2(300.0, 0.0), vec2(200.0, 100.0)),
    );
    let frame = frame(
        &[(1, editor), (2, popover), (3, child), (4, beside)],
        vec2(800.0, 600.0),
    );
    let surfaces: Vec<Surface> = frame.layers.iter().map(|layer| layer.surface).collect();
    assert_eq!(
        surfaces,
        vec![
            Surface::Screen {
                instance: 7,
                surface: 1
            },
            Surface::Screen {
                instance: 7,
                surface: 3
            },
            Surface::Root(0),
            Surface::Anchored {
                instance: 7,
                surface: 9
            },
            Surface::Screen {
                instance: 7,
                surface: 2
            },
        ],
        "the child placed in the popover goes above it, and the pane beside it stays under the host"
    );
    let anchored = &frame.layers[3];
    assert_eq!(
        (anchored.bounds(), anchored.clip),
        (
            Rect::from_min_max(pos2(20.0, 20.0), pos2(80.0, 70.0)),
            Rect::from_min_max(Pos2::ZERO, pos2(60.0, 50.0))
        ),
        "the web keeps an anchored view's piece to the view's bounds"
    );
}

use super::*;

#[test]
fn popups_have_no_chrome() {
    let (mut space, now) = space();
    let mut opened = Vec::new();
    for role in [WindowRole::Popup, WindowRole::Normal] {
        let window = WindowId::next();
        space.request(&WindowRequest::Open {
            window,
            placement: WindowPlacement::Over {
                window: WindowId::MAIN,
                at: pos2(40.0 + 500.0 * opened.len() as f32, 200.0),
            },
            size: SMALL,
            role,
        });
        send(&mut space, now, []);
        opened.push(window);
    }
    let frame = space.scene(&[], &view(now));
    let framed = |window: WindowId| {
        frame
            .panels
            .iter()
            .find(|panel| panel.window == window)
            .expect("each window has a panel")
            .chrome
            .is_some()
    };
    assert!(!framed(opened[0]), "a popup is drawn bare");
    assert!(framed(opened[1]), "a window has its chrome");
    for (window, has) in [(opened[0], false), (opened[1], true)] {
        let panel = *space.panel(window).unwrap();
        let above = screen(&space, panel.world(pos2(SMALL.x / 2.0, -12.0)));
        let picked = crate::chrome::pick(
            &space.panels,
            space.head.ray(PLATFORM, above.to_vec2()),
            None,
        );
        assert_eq!(
            picked.is_some_and(|(_, hit)| hit.window == window),
            has,
            "only a framed window's chrome takes presses"
        );
    }
}

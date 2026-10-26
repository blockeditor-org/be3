use super::the_chrome_around_a_curved_panel::window;
use super::*;

fn resized(incoming: &[Incoming]) -> Vec<(WindowId, Vec2)> {
    incoming
        .iter()
        .filter_map(|incoming| match incoming {
            Incoming::Window(WindowReport::Resized { window, size }) => Some((*window, *size)),
            _ => None,
        })
        .collect()
}

#[test]
fn dragging_a_side_of_the_main_panel_resizes_its_window_and_keeps_its_scale() {
    let (mut space, now, context) = painted_space();
    let start = *main(&space);
    let out = (chrome::GAP + chrome::THICKNESS / 2.0) * start.chrome.scale / start.metres_per_point;
    let side = pos2(start.size.x + out, start.size.y / 2.0);
    assert_eq!(
        chrome::part_at(&start, side, false),
        Some(Part::Resize(chrome::Side::Right))
    );
    let grab = screen(&space, start.world(side));
    let wider = screen(&space, start.world(side + vec2(-120.0, 40.0)));
    let sent = send(
        &mut space,
        now,
        [
            Input::PointerMoved(grab),
            button(grab, true),
            Input::PointerMoved(wider),
        ],
    );
    assert_eq!(delivered(&sent), [], "the app sees nothing of the side");
    let size = vec2(start.size.x - 120.0, start.size.y);
    assert_eq!(
        resized(&sent),
        [(WindowId::MAIN, size)],
        "the main window is told its new size at once, its height kept"
    );
    assert_eq!(
        space.cursor_icon(&app_frame(CursorIcon::Text)),
        CursorIcon::ResizeHorizontal
    );
    let panel = *main(&space);
    assert_eq!(panel.size, size);
    assert_eq!(
        panel.metres_per_point, start.metres_per_point,
        "a resize keeps the panel's scale"
    );
    assert!(
        close(panel.world(Pos2::ZERO), start.world(Pos2::ZERO)),
        "the left side stays"
    );
    assert_eq!(
        space.views()[0].size,
        PLATFORM,
        "the platform window, the mirror, keeps its size"
    );

    let windows = [window(&context, WindowId::MAIN, size, "Resized")];
    assert_scene(
        &mut space,
        now,
        &windows,
        "dragging_a_side_of_the_main_panel_resizes_its_window_and_keeps_its_scale",
    );

    let released = send(&mut space, now, [button(wider, false)]);
    assert_eq!(delivered(&released), []);
    assert_eq!(placed(&released).len(), 1, "letting go reports the place");
    assert_eq!(space.mouse.resize, None);
    assert_eq!(space.mouse.gesture, None);

    let panel = *main(&space);
    let out = (chrome::GAP + chrome::THICKNESS / 2.0) * panel.chrome.scale / panel.metres_per_point;
    let bottom = panel.world(pos2(panel.size.x * 0.2, panel.size.y + out));
    let lower = panel.world(pos2(panel.size.x * 0.2, panel.size.y + out + 300.0));
    let hand = right_hand();
    drive(
        &mut space,
        now,
        [hand_pose(hand, bottom), trigger(Hand::Right, 1.0)],
    );
    let moved = drive(&mut space, now, [hand_pose(hand, lower)]);
    assert_eq!(
        resized(&moved),
        [(WindowId::MAIN, vec2(size.x, size.y + 300.0))],
        "a laser drags the bottom side down"
    );
    let let_go = drive(&mut space, now, [trigger(Hand::Right, 0.0)]);
    assert_eq!(placed(&let_go).len(), 1);
    assert!(touches(&let_go).is_empty(), "and the app is never touched");
}

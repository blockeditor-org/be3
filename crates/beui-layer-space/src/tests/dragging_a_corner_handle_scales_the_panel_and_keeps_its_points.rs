use super::*;

#[test]
fn dragging_a_corner_handle_scales_the_panel_and_keeps_its_points() {
    let (mut space, now) = space();
    let start = *main(&space);
    let kept = start.world(Pos2::ZERO);
    let out = (chrome::GAP + chrome::THICKNESS / 2.0) * start.chrome.scale / start.metres_per_point;
    let handle = pos2(start.size.x + out, start.size.y + out);
    assert_eq!(
        chrome::part_at(&start, handle, false),
        Some(Part::Scale(Corner::BottomRight))
    );
    assert!(!start.contains(handle), "the handle is outside the window");
    let grab = screen(&space, start.world(handle));
    let nearer = handle + (handle - Pos2::ZERO) * -0.2;
    let to = screen(&space, start.world(nearer));

    let sent = send(
        &mut space,
        now,
        [
            Input::PointerMoved(grab),
            button(grab, true),
            Input::PointerMoved(to),
        ],
    );
    assert_eq!(delivered(&sent), [], "the app sees nothing of the handle");
    assert_eq!(reports(&sent), [], "and is told of no new size");
    assert_eq!(
        space.cursor_icon(&app_frame(CursorIcon::Text)),
        CursorIcon::ResizeNwSe,
        "the cursor shows the handle resizes"
    );
    let panel = *main(&space);
    assert_eq!(panel.size, start.size, "the window keeps its points");
    assert!(
        (panel.metres_per_point / start.metres_per_point - 0.8).abs() < 0.01,
        "the panel shrank by a fifth: {}",
        panel.metres_per_point / start.metres_per_point
    );
    assert!(close(panel.world(Pos2::ZERO), kept), "the far corner stays");
    assert!(
        (screen(&space, panel.world(handle)) - to).length() < 1.0,
        "the handle stays under the cursor"
    );

    let sharper =
        |panel: &Panel| crate::scale::wanted(panel, &[space.head.view(PLATFORM)]).unwrap();
    assert!(
        (sharper(&panel) / sharper(&start) - 0.8).abs() < 0.01,
        "its texture gets as many fewer pixels per point as it shrank"
    );

    let released = send(&mut space, now, [button(to, false)]);
    assert_eq!(delivered(&released), []);
    assert_eq!(placed(&released).len(), 1, "the new place is reported");
    assert_eq!(space.mouse.gesture, None);
}

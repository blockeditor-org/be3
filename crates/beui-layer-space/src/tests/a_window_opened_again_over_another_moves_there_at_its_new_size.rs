use super::*;
use crate::OVER_NEARER;

#[test]
fn a_window_opened_again_over_another_moves_there_at_its_new_size() {
    let (mut space, now) = space();
    let (window, _) = open(
        &mut space,
        now,
        WindowPlacement::Over {
            window: WindowId::MAIN,
            at: pos2(64.0, 48.0),
        },
    );
    let at = pos2(20.0, 120.0);
    let size = vec2(SMALL.x * 2.0, SMALL.y * 3.0);
    space.request(&WindowRequest::Open {
        window,
        placement: WindowPlacement::Over {
            window: WindowId::MAIN,
            at,
        },
        size,
        role: WindowRole::Normal,
    });
    let incoming = send(&mut space, now, []);
    let main = main(&space).clone();
    let panel = space.panel(window).unwrap();
    assert_eq!(
        panel.size, size,
        "it takes the size it is opened again with"
    );
    assert!(
        close(
            panel.world(Pos2::ZERO),
            main.world(at) + main.normal() * OVER_NEARER
        ),
        "and moves its top left corner in front of the main panel's point {at:?}"
    );
    assert!(
        reports(&incoming).iter().any(|report| matches!(
            report,
            WindowReport::Resized { window: resized, size: reported }
                if *resized == window && *reported == size
        )),
        "and reports the new size"
    );
}

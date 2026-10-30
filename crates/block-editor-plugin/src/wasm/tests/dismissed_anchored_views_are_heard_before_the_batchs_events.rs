use block_plugin_api::{Input, InputBatch};

use super::*;

#[test]
fn dismissed_anchored_views_are_heard_before_the_batchs_events() {
    let mut rig = Rig::new(Some(space()));
    rig.open(vec![layer(
        1,
        0,
        Rect::from_min_size(pos2(220.0, 130.0), vec2(60.0, 40.0)),
    )]);
    let commit = rig.draw();
    let (menu, _) = Rig::anchored(&commit)[0];
    let moved = || TimedInput::from(Input::PointerMoved(pos2(30.0, 40.0)));
    rig.screens.deliver(Delivery::Input(InputBatch {
        screen: SCREEN,
        events: vec![moved()],
        every_sample: false,
        dismissed: vec![ViewId(1), ViewId(7)],
    }));
    rig.screens.deliver(Delivery::Input(InputBatch {
        screen: menu,
        events: vec![moved()],
        every_sample: false,
        dismissed: Vec::new(),
    }));
    assert_eq!(
        rig.shared.seen.borrow().heard,
        [
            "dismissed [ViewId(1), ViewId(7)]",
            "input on ViewId(0) at PointerMoved(Pos2 { x: 30.0, y: 40.0 })",
            "input on ViewId(1) at PointerMoved(Pos2 { x: 30.0, y: 40.0 })",
        ],
        "input on the anchored view reaches it as input on its own screen"
    );
}

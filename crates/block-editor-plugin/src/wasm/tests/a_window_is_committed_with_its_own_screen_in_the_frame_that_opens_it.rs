use super::*;

#[test]
fn a_window_is_committed_with_its_own_screen_in_the_frame_that_opens_it() {
    let mut rig = Rig::new(None);
    let opener = ViewRef {
        instance: INSTANCE,
        view: ViewId::MAIN,
    };
    *rig.shared.window.borrow_mut() = Some(block_plugin_api::WindowOpen {
        view: ViewId(9),
        screen: ScreenId(0),
        opener,
        placement: block_plugin_api::WindowPlacement::AtPointer,
        size: Size {
            width: 200.0,
            height: 100.0,
        },
        role: block_plugin_api::WindowRole::Normal,
    });
    let commit = rig.draw();

    let open = commit
        .window_requests
        .iter()
        .find_map(|request| match request {
            block_plugin_api::WindowRequest::Open(open) => Some(open.clone()),
            _ => None,
        })
        .expect("the window is asked of the host");
    assert!(
        open.screen.is_opened(),
        "the plugin numbers the window's screen itself: {:?}",
        open.screen
    );
    let placed = *rig
        .screens
        .layout()
        .placement(open.screen)
        .expect("the plugin lists the window's screen in its layout");
    assert_eq!(
        (placed.view, placed.width, placed.height),
        (ViewId(9), 400, 200),
        "at the size it opened at, at its opener's raster"
    );
    assert!(
        rig.surfaces.take_relaid(),
        "the runtime sends the host the layout with the window before the commit"
    );
    assert!(
        commit.reports.iter().any(|report| report.screen == open.screen),
        "the window is reported in the commit that opens it"
    );
    let damage = commit.presented.expect("the frame presented").damage;
    assert!(
        damage.iter().any(|damage| damage.screen == open.screen),
        "and painted in it: {damage:?}"
    );

    let mut window = rig.requests[0].clone();
    window.screen = open.screen;
    window.view = ViewId(9);
    window.placement = ViewPlacement::Host(block_plugin_api::HostView::Window);
    window.metrics.logical = Size {
        width: 200.0,
        height: 100.0,
    };
    window.metrics.pixel_width = 400;
    window.metrics.pixel_height = 200;
    rig.requests.push(window);
    rig.draw();
    assert_eq!(
        rig.screens
            .layout()
            .placement(open.screen)
            .map(|placement| placement.surface),
        Some(placed.surface),
        "once the host asks for the window by that screen, it keeps its surface"
    );
}

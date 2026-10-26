use super::the_chrome_around_a_curved_panel::window;
use super::*;

fn titled(
    context: &Context,
    id: WindowId,
    size: Vec2,
    title: &str,
) -> beui_core::context::WindowOutput {
    let mut output = window(context, id, size, title);
    output.window.appearance.title = Some(title.to_owned());
    output
}

#[test]
fn each_panel_shows_its_window_s_title_and_dragging_the_title_carries_it() {
    let (mut space, now, context) = painted_space();
    let notes = open(
        &mut space,
        now,
        WindowPlacement::Over {
            window: WindowId::MAIN,
            at: pos2(80.0, 260.0),
        },
    )
    .0;
    let narrow = open(
        &mut space,
        now,
        WindowPlacement::Over {
            window: WindowId::MAIN,
            at: pos2(500.0, 300.0),
        },
    )
    .0;
    space.request(&WindowRequest::Resize {
        window: narrow,
        size: vec2(140.0, 150.0),
    });
    let windows = [
        titled(&context, WindowId::MAIN, PLATFORM, "Workspace"),
        titled(&context, notes, SMALL, "Notes"),
        titled(
            &context,
            narrow,
            vec2(140.0, 150.0),
            "A title far too long for its window",
        ),
    ];
    space.compose(&windows, &[], &view(now));
    send(&mut space, now, []);
    for window in [WindowId::MAIN, notes, narrow] {
        assert!(
            space.panel(window).unwrap().chrome.title > 0.0,
            "{window:?} has its title measured"
        );
    }
    assert_scene(
        &mut space,
        now,
        &windows,
        "each_panel_shows_its_window_s_title_and_dragging_the_title_carries_it",
    );

    let title = on_part(&space, notes, Part::Title);
    let sent = send(
        &mut space,
        now,
        [Input::PointerMoved(title), button(title, true)],
    );
    assert_eq!(delivered(&sent), [], "the app sees nothing of the title");
    assert!(
        space
            .mouse
            .carry
            .as_ref()
            .is_some_and(|carry| carry.window == notes),
        "pressing the title starts carrying its panel"
    );
    assert_eq!(
        space.cursor_icon(&app_frame(CursorIcon::Text)),
        CursorIcon::Grabbing
    );
    let start = space.panel(notes).unwrap().pose.position;
    let aside = title + vec2(80.0, 20.0);
    send(&mut space, now, [Input::PointerMoved(aside)]);
    assert!(
        space.panel(notes).unwrap().pose.position.distance(start) > 0.02,
        "and moving the mouse carries it"
    );
    let released = send(&mut space, now, [button(aside, false)]);
    assert_eq!(placed(&released).len(), 1);
    assert!(space.mouse.carry.is_none());

    let untitled = window(&context, notes, SMALL, "Notes");
    space.compose(
        &[titled(&context, WindowId::MAIN, PLATFORM, "Workspace"), untitled],
        &[],
        &view(now),
    );
    assert_eq!(
        space.panel(notes).unwrap().chrome.title,
        0.0,
        "a window that drops its title loses it from its chrome"
    );
}

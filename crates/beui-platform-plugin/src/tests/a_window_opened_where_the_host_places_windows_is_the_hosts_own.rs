use super::*;

#[test]
fn a_window_opened_where_the_host_places_windows_is_the_hosts_own() {
    let _root = RootScope::new();
    let reports = Rc::new(RefCell::new(Vec::new()));
    let heard = Rc::clone(&reports);
    let pointer = Rc::new(Cell::new(None));
    let seen = Rc::clone(&pointer);
    let mut surface = launched(
        &EditorHost::default(),
        Windowed::new(move |id| {
            let heard = Rc::clone(&heard);
            match id.view.is_main() {
                true => view! {
                    <Window
                        size={vec2(200.0, 100.0)}
                        on_report={move |report: WindowReport| heard.borrow_mut().push(report)}
                    >|_window: WindowHandle|
                        <styled::Frame />
                    </Window>
                },
                _ => view! {
                    <styled::Frame />
                },
            }
        })
        .hooked(move |context| {
            seen.set(context.input(|input| {
                input
                    .pointer
                    .interact_pos()
                    .map(|pos| (input.pointer.window, pos))
            }))
        }),
    );
    let mut frame = whole(400.0, 300.0);
    frame.windows = true;
    let asks = one(&mut surface, &frame).asks;
    let opened = asks.iter().find_map(|ask| match ask {
        block_plugin_api::WindowRequest::Open(open) => Some(open.clone()),
        _ => None,
    });
    let opened = opened.expect("the window is asked of the host");
    assert_eq!(
        (opened.opener.view, opened.placement.clone(), opened.size,),
        (
            ViewId::MAIN,
            block_plugin_api::WindowPlacement::AtPointer,
            block_plugin_api::Size {
                width: 200.0,
                height: 100.0
            },
        ),
        "from the region it was opened in, at the pointer, at its size, with its role"
    );
    let mut window = whole(200.0, 100.0);
    window.view = block_editor_plugin::View::window(opened.view);
    window.windows = true;
    window.host_place = Some("desktop 10 20".to_owned());
    surface.frame(&[frame.clone(), window.clone()]);
    surface.frame(&[frame.clone(), window.clone()]);
    let shown = surface
        .regions()
        .window(window.id())
        .expect("the host's window region shows the app's window");
    assert_eq!(
        window_view(shown),
        opened.view,
        "the very window it asked for"
    );
    assert!(
        reports.borrow().iter().any(|report| matches!(
            report,
            WindowReport::Placed { placement, .. } if placement.0 == "desktop 10 20"
        )),
        "the host's placement reaches the window: {:?}",
        reports.borrow()
    );
    surface.input(
        &frame,
        &TimedInput::from(Input::PointerButton {
            pos: pos2(5.0, 5.0),
            button: block_plugin_api::PointerButton::Primary,
            pressed: true,
        }),
        None,
    );
    surface.input(
        &frame,
        &TimedInput::from(Input::PointerMoved(pos2(30.0, 40.0))),
        Some(window.id()),
    );
    surface.frame(&[frame.clone(), window.clone()]);
    assert_eq!(
        pointer.get(),
        Some((shown, pos2(30.0, 40.0))),
        "a press held in the region goes on over the window the host names, in its points"
    );
    surface.frame(std::slice::from_ref(&frame));
    assert!(
        reports
            .borrow()
            .iter()
            .any(|report| matches!(report, WindowReport::Closed(_))),
        "a window region the host stops showing closes the window"
    );
}

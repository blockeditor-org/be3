use super::*;

#[test]
fn a_window_the_app_opens_is_painted_in_the_frame_that_opens_it() {
    let _root = RootScope::new();
    let mut surface = launched(
        &EditorHost::default(),
        Windowed::new(|id| match id.view.is_main() {
            true => view! {
                <Window size={vec2(200.0, 100.0)}>|_window: WindowHandle|
                    <styled::Frame fill=styled::ColorRole::Accent />
                </Window>
            },
            false => view! {
                <styled::Frame />
            },
        }),
    );
    let mut frame = whole(400.0, 300.0);
    frame.windows = true;
    let opened = one(&mut surface, &frame)
        .asks
        .into_iter()
        .find_map(|ask| match ask {
            block_plugin_api::WindowRequest::Open(open) => Some(open),
            _ => None,
        })
        .expect("the window is asked of the host");

    let mut window = whole(200.0, 100.0);
    window.view = block_editor_plugin::View::window(opened.view);
    window.windows = true;
    let framed = surface.frame_opened(std::slice::from_ref(&window));
    assert!(
        framed[0].changed,
        "the window is drawn with the frame that opened it, before the host asks for it"
    );
    let shown = surface
        .window(window.id())
        .expect("the window's view is the app's window at once");
    assert_eq!(window_view(shown), opened.view);
    let painting = surface
        .painting(window.id())
        .expect("the window paints in the frame that opened it");
    assert_eq!(painting.frame.size, [200, 100]);

    surface.frame(&[frame.clone(), window.clone()]);
    assert_eq!(
        surface.window(window.id()),
        Some(shown),
        "the host asking for the window goes on showing the same window"
    );
}

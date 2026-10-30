use super::*;

#[test]
fn a_window_the_app_floats_inside_a_view_stays_in_that_view() {
    let _root = RootScope::new();
    let floated = Rc::new(Cell::new(None));
    let seen = Rc::clone(&floated);
    let mut surface = launched(
        &EditorHost::default(),
        Windowed::new(move |_| {
            let seen = Rc::clone(&seen);
            view! {
                <Window size={vec2(10.0, 10.0)}>|window: WindowHandle| {
                    seen.set(Some(window.is_window()));
                    view! {
                        <styled::Frame />
                    }
                }</Window>
            }
        }),
    );
    let shown = whole(40.0, 30.0);
    one(&mut surface, &shown);
    one(&mut surface, &shown);
    assert_eq!(
        floated.get(),
        Some(false),
        "a window opened without one of the platform's placements is drawn inside the region it was opened in"
    );
    let windows = surface
        .regions()
        .runner
        .output()
        .map(|output| output.windows.len())
        .unwrap_or_default();
    assert_eq!(
        windows, 2,
        "only the main window and the region's are shown"
    );
}

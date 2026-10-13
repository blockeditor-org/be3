use super::*;

struct Asking(Rc<Cell<bool>>);

impl App for Asking {
    fn update(&mut self, context: &Context, _rect: Rect) {
        if self.0.take() {
            context.request_keyboard();
        }
    }
}

#[test]
fn a_keyboard_asked_for_in_a_region_is_asked_of_the_host_once() {
    let asking = Rc::new(Cell::new(false));
    let mut surface = PluginSurface::launch(
        EditorHost::default(),
        beui::context(),
        Asking(asking.clone()),
    );
    let whole = Rect::from_min_size(pos2(0.0, 0.0), vec2(40.0, 30.0));
    assert!(!surface.update(&region(whole, [40, 30])).show_keyboard);

    asking.set(true);
    assert!(
        surface.update(&region(whole, [40, 30])).show_keyboard,
        "the frame asks the host to show the keyboard"
    );
    assert!(
        !surface.update(&region(whole, [40, 30])).show_keyboard,
        "and only once"
    );
}

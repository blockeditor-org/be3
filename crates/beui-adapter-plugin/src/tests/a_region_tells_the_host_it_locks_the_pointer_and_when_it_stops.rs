use super::*;

struct Locking(Rc<Cell<u8>>);

impl App for Locking {
    fn update(&mut self, context: &Context, _rect: Rect) {
        match self.0.get() {
            0 => context.set_pointer_locked(true),
            1 => context.set_pointer_locked(false),
            2 => context.close_window(),
            _ => panic!("out of cheese"),
        }
    }
}

#[test]
fn a_region_tells_the_host_it_locks_the_pointer_and_when_it_stops() {
    let step = Rc::new(Cell::new(0));
    let whole = Rect::from_min_size(pos2(0.0, 0.0), vec2(40.0, 30.0));
    let mut surface = PluginSurface::launch(
        EditorHost::default(),
        beui::context(),
        Locking(step.clone()),
    );
    let frame = surface.update(&region(whole, [40, 30]));
    assert!(frame.window.pointer_locked);
    assert_eq!(frame.stopped, None);

    step.set(1);
    assert!(
        !surface
            .update(&region(whole, [40, 30]))
            .window
            .pointer_locked
    );

    step.set(2);
    assert_eq!(
        surface.update(&region(whole, [40, 30])).stopped,
        Some(block_plugin_api::Stopped::Closed),
        "an app that closed its window says so rather than going blank"
    );
    assert_eq!(
        surface.update(&region(whole, [40, 30])).stopped,
        Some(block_plugin_api::Stopped::Closed)
    );

    let mut surface = PluginSurface::launch(
        EditorHost::default(),
        beui::context(),
        Locking(Rc::new(Cell::new(3))),
    );
    assert_eq!(
        surface.update(&region(whole, [40, 30])).stopped,
        Some(block_plugin_api::Stopped::Failed(
            "out of cheese".to_owned()
        ))
    );
}

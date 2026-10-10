use super::*;

struct Fullscreen(Rc<Cell<bool>>);

impl App for Fullscreen {
    fn update(&mut self, context: &Context, _rect: Rect) {
        context.set_fullscreen(self.0.get());
    }
}

#[test]
fn a_headless_window_fills_its_screen_when_the_app_goes_fullscreen() {
    let fullscreen = Rc::new(Cell::new(false));
    let mut driven = Driven::with_app(Fullscreen(fullscreen.clone()), true);
    let state = driven.ask(&["state"]).expect("the state is read");
    assert!(state.starts_with("window 400x300 "), "{state}");

    fullscreen.set(true);
    driven.ask(&["settle"]).expect("the app settles");
    let state = driven.ask(&["state"]).expect("the state is read");
    assert!(
        state.starts_with("window 1000x600 at scale 1 (headless; windowed 400x300, screen 1000x600), fullscreen yes"),
        "the window grows to the screen:\n{state}"
    );

    fullscreen.set(false);
    driven.ask(&["settle"]).expect("the app settles");
    let state = driven.ask(&["state"]).expect("the state is read");
    assert!(state.starts_with("window 400x300 "), "it shrinks back:\n{state}");
}

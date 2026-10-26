use super::*;

struct Fullscreen(ReadSignal<bool>);

impl App for Fullscreen {
    fn update(&mut self, context: &Context, _rect: Rect) {
        context.set_fullscreen(self.0.get_untracked());
    }
}

#[test]
fn a_headless_window_fills_its_screen_when_the_app_goes_fullscreen() {
    let _root = crate::reactive::RootScope::new();
    let fullscreen = create_signal(false);
    let mut driven = headless(Fullscreen(fullscreen.read_only()));
    let state = driven.ask(&["app", "state"]).expect("the state is read");
    assert!(state.starts_with("window 400x300 "), "{state}");

    fullscreen.set(true);
    driven.ask(&["settle"]).expect("the app settles");
    let state = driven.ask(&["app", "state"]).expect("the state is read");
    assert!(
        state.starts_with("window 1000x600 at scale 1, fullscreen yes"),
        "the window grows to the screen:\n{state}"
    );

    fullscreen.set(false);
    driven.ask(&["settle"]).expect("the app settles");
    let state = driven.ask(&["app", "state"]).expect("the state is read");
    assert!(
        state.starts_with("window 400x300 "),
        "it shrinks back:\n{state}"
    );
}

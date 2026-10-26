use super::*;

struct Animating(Signal<Option<std::time::Instant>>);

impl App for Animating {
    fn update(&mut self, context: &Context, _rect: Rect) {
        self.0.set(Some(context.now()));
        context.request_repaint();
    }
}

#[test]
fn a_stopped_clock_lets_an_animating_app_settle_and_pause_moves_it() {
    let _root = crate::reactive::RootScope::new();
    let now = create_signal(None);
    let mut driven = headless(Animating(now));

    driven
        .ask(&["clock", "stop"])
        .expect("the clock stops and the app settles");
    let stopped = now.get_untracked().expect("a frame ran");
    driven
        .ask(&["settle"])
        .expect("an animating app settles while time stands still");
    let state = driven.ask(&["app", "state"]).expect("the state is read");
    assert!(state.contains("clock stopped"), "{state}");

    driven.ask(&["pause", "500"]).expect("time passes");
    let later = now.get_untracked().expect("frames ran");
    assert!(
        later.duration_since(stopped) >= std::time::Duration::from_millis(500),
        "the app saw half a second pass"
    );
}

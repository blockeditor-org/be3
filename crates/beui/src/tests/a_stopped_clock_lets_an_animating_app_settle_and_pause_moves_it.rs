use super::*;

struct Animating(Rc<Cell<Option<std::time::Instant>>>);

impl App for Animating {
    fn update(&mut self, context: &Context, _rect: Rect) {
        self.0.set(Some(context.now()));
        context.request_repaint();
    }
}

#[test]
fn a_stopped_clock_lets_an_animating_app_settle_and_pause_moves_it() {
    let now = Rc::new(Cell::new(None));
    let mut driven = Driven::with_app(Animating(now.clone()), true);

    driven
        .ask(&["clock", "stop"])
        .expect("the clock stops and the app settles");
    let stopped = now.get().expect("a frame ran");
    driven
        .ask(&["settle"])
        .expect("an animating app settles while time stands still");
    let state = driven.ask(&["state"]).expect("the state is read");
    assert!(state.contains("clock stopped"), "{state}");

    driven.ask(&["pause", "500"]).expect("time passes");
    let later = now.get().expect("frames ran");
    assert!(
        later.duration_since(stopped) >= std::time::Duration::from_millis(500),
        "the app saw half a second pass"
    );
}

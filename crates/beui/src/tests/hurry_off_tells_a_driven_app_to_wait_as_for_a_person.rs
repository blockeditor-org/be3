use super::*;

struct Hurrying(Signal<Vec<bool>>);

impl App for Hurrying {
    fn update(&mut self, context: &Context, _rect: Rect) {
        let hurried = context.hurried();
        self.0.update(|seen| seen.push(hurried));
    }
}

#[test]
fn hurry_off_tells_a_driven_app_to_wait_as_for_a_person() {
    let _root = crate::reactive::RootScope::new();
    let seen = create_signal(Vec::new());
    let mut driven = headless(Hurrying(seen));

    driven.ask(&["settle"]).expect("the app settles");
    assert_eq!(
        seen.get_untracked().last(),
        Some(&true),
        "a driven app hurries"
    );

    driven.ask(&["hurry", "off"]).expect("hurrying stops");
    driven.ask(&["settle"]).expect("the app settles");
    assert_eq!(seen.get_untracked().last(), Some(&false));
    let state = driven.ask(&["state"]).expect("the state is read");
    assert!(state.contains("hurry off"), "{state}");

    driven.ask(&["hurry", "on"]).expect("hurrying starts again");
    assert_eq!(seen.get_untracked().last(), Some(&true));
    assert!(driven.ask(&["hurry", "maybe"]).is_err());
}

use super::*;
use crate::reactive::{Text, build, view};
use crate::unstyled::Button;

#[test]
fn on_interacted_runs_after_the_frame_s_input_is_dispatched() {
    let clicks = std::rc::Rc::new(std::cell::Cell::new(0));
    let sink = clicks.clone();
    let button = std::rc::Rc::new(std::cell::Cell::new(None));
    let held = button.clone();
    let mut document = build(move || {
        let node = view! {
            <Button on_click={move || sink.set(sink.get() + 1)}>
                <Text string="go" />
            </Button>
        };
        held.set(Some(node));
        node
    });
    let seen = std::rc::Rc::new(std::cell::RefCell::new(Vec::new()));
    let (watched, record) = (clicks.clone(), seen.clone());
    document.on_interacted(move || record.borrow_mut().push(watched.get()));
    let button = button.get().expect("the button was built");
    let mut harness = Harness::new(document);
    harness.frame(Vec::new());

    harness.click(harness.center(button));

    assert_eq!(clicks.get(), 1);
    assert_eq!(
        seen.borrow().last(),
        Some(&1),
        "the hook sees the click delivered in the frame it runs in"
    );
}

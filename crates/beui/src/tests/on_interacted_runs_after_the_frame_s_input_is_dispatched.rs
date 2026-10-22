use super::*;
use crate::primitives::*;
use crate::reactive::*;
use crate::unstyled::Button;

#[test]
fn on_interacted_runs_after_the_frame_s_input_is_dispatched() {
    let _root = crate::reactive::RootScope::new();
    let clicks = create_signal(0);
    let button = create_signal(None);
    let mut document = build_node(move || {
        let node = view! {
            <Button on_click={move || clicks.update(|clicks| *clicks += 1)}>
                <Text>"go"</Text>
            </Button>
        };
        button.set(Some(node));
        node
    });
    let seen = create_signal(Vec::new());
    document.on_interacted(move || seen.update(|seen| seen.push(clicks.get_untracked())));
    let button = button.get_untracked().expect("the button was built");
    let mut harness = Harness::new(document);
    harness.frame(Vec::new());

    harness.click(harness.center(button));

    assert_eq!(clicks.get_untracked(), 1);
    assert_eq!(
        seen.get_untracked().last(),
        Some(&1),
        "the hook sees the click delivered in the frame it runs in"
    );
}

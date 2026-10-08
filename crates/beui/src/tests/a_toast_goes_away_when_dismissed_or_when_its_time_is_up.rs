use super::*;
use crate::styled::{TOAST_DURATION, Toast, Toasts};

#[test]
fn a_toast_goes_away_when_dismissed_or_when_its_time_is_up() {
    let toast = |id: u64| Toast {
        id,
        message: format!("Toast {id}"),
        danger: id == 1,
    };
    let document = build(move || {
        let area = NodeRef::new();
        let (toasts, set_toasts) = create_signal(vec![toast(1), toast(2)]);
        view! {
            <Frame @node_ref=&area width={VIEWPORT.x} height={VIEWPORT.y}>
                <Toasts
                    anchor={area.clone()}
                    toasts={toasts}
                    on_dismiss={move |id: u64| {
                        set_toasts.update(|toasts| toasts.retain(|toast| toast.id != id))
                    }}
                />
            </Frame>
        }
    });
    let mut harness = Harness::new(document);
    harness.context.set_test_ids_published(true);
    let output = harness.frame(Vec::new());
    let first = output
        .test_id_rect("toast.1")
        .expect("the first toast shows");
    let second = output
        .test_id_rect("toast.2")
        .expect("the second toast shows");
    assert!(first.bottom() <= second.top(), "the toasts stack in order");
    assert!(
        second.right() > VIEWPORT.x - 40.0 && second.bottom() > VIEWPORT.y - 40.0,
        "the toasts sit in the bottom corner, at {second:?}"
    );

    let dismiss = output
        .test_id_rect("toast.1.dismiss")
        .expect("a toast has a dismiss button")
        .center();
    harness.click(dismiss);
    let output = harness.frame(Vec::new());
    assert!(
        output.test_id_rect("toast.1").is_none(),
        "the dismissed toast is gone"
    );
    assert!(
        output.test_id_rect("toast.2").is_some(),
        "the other toast stays"
    );

    harness.advance(TOAST_DURATION);
    harness.frame(Vec::new());
    let output = harness.frame(Vec::new());
    assert!(
        output.test_id_rect("toast.2").is_none(),
        "a toast goes when its time is up"
    );
}

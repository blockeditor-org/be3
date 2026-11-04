use super::*;
use crate::primitives::Frame;
use crate::styled::*;

#[test]
fn an_error_toast_stays_until_it_is_dismissed() {
    let toast = |id: u64, danger: bool| Toast {
        id,
        message: format!("Toast {id}"),
        danger,
        ..Toast::default()
    };
    let document = build_node(move || {
        let area = NodeRef::new();
        let toasts = create_signal(vec![toast(1, true), toast(2, false)]);
        view! {
            <Frame @node_ref=&area width={VIEWPORT.x} height={VIEWPORT.y}>
                <Toasts
                    anchor={area}
                    toasts={toasts}
                    on_dismiss={move |id: u64| {
                        toasts.update(|toasts| toasts.retain(|toast| toast.id != id))
                    }}
                />
            </Frame>
        }
    });
    let mut harness = Harness::new(document);
    harness.frame(Vec::new());

    harness.advance(TOAST_DURATION * 4);
    harness.frame(Vec::new());
    let output = harness.frame(Vec::new());
    assert!(
        output.test_id_rect("toast.2").is_none(),
        "an ordinary toast goes when its time is up"
    );
    let dismiss = output
        .test_id_rect("toast.1.dismiss")
        .expect("an error toast outlasts its time, so it cannot be missed")
        .center();

    harness.click(dismiss);
    let output = harness.frame(Vec::new());
    assert!(
        output.test_id_rect("toast.1").is_none(),
        "dismissing it is what takes it away"
    );
}

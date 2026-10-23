use super::*;
use crate::primitives::Frame;
use crate::styled::*;

#[test]
fn a_toast_with_actions_reports_them_and_stays_until_answered() {
    let _root = crate::reactive::RootScope::new();
    let toast = Toast {
        id: 7,
        title: "Mia Chen".to_owned(),
        message: "Lunch at noon?".to_owned(),
        actions: vec![ToastAction {
            key: "reply".to_owned(),
            label: "Reply".to_owned(),
        }],
        activates: true,
        sticky: true,
        ..Toast::default()
    };
    let heard = create_signal::<Vec<String>>(Vec::new());
    let document = build_node(move || {
        let area = NodeRef::new();
        let toasts = create_signal(vec![toast.clone()]);
        view! {
            <Frame @node_ref=&area width={VIEWPORT.x} height={VIEWPORT.y}>
                <Toasts
                    anchor={area}
                    toasts={toasts}
                    on_dismiss={|_: u64| {}}
                    on_action={move |(id, key): (u64, String)| {
                        heard.update(|heard| heard.push(format!("{id} {key}")))
                    }}
                    on_activate={move |id: u64| heard.update(|heard| heard.push(format!("{id} opened")))}
                />
            </Frame>
        }
    });
    let mut harness = Harness::new(document);
    let output = harness.frame(Vec::new());
    let reply = output
        .test_id_rect("toast.7.action.reply")
        .expect("the toast shows its action")
        .center();
    let body = output
        .test_id_rect("toast.7.activate")
        .expect("a toast that activates can be clicked")
        .center();
    harness.click(reply);
    harness.frame(Vec::new());
    harness.click(body);
    harness.frame(Vec::new());
    assert_eq!(heard.get_untracked(), vec!["7 reply", "7 opened"]);

    harness.advance(TOAST_DURATION * 2);
    harness.frame(Vec::new());
    let output = harness.frame(Vec::new());
    assert!(
        output.test_id_rect("toast.7").is_some(),
        "a sticky toast waits to be answered"
    );
}

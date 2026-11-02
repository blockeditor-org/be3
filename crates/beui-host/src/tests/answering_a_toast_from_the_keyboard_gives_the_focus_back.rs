use super::*;
use crate::primitives::Frame;
use crate::styled::List as Stack;
use crate::styled::*;

#[test]
fn answering_a_toast_from_the_keyboard_gives_the_focus_back() {
    let _root = crate::reactive::RootScope::new();
    let toast = Toast {
        id: 7,
        title: "Mia Chen".to_owned(),
        actions: vec![ToastAction {
            key: "reply".to_owned(),
            label: "Reply".to_owned(),
        }],
        sticky: true,
        ..Toast::default()
    };
    let edit = NodeRef::new();
    let document = build_node(move || {
        let area = NodeRef::new();
        let other = Toast {
            id: 8,
            title: "Sam".to_owned(),
            sticky: true,
            ..Toast::default()
        };
        let toasts = create_signal(vec![other, toast.clone()]);
        view! {
            <Frame @node_ref=&area width={VIEWPORT.x} height={VIEWPORT.y}>
                <Stack spacing=Space::Md>
                    <Button @node_ref=&edit label="Edit" on_click={|| {}} />
                    <Toasts
                        anchor={area}
                        toasts={toasts}
                        on_dismiss={|_: u64| {}}
                        on_action={move |(id, _): (u64, String)| {
                            toasts.update(|toasts| toasts.retain(|toast| toast.id != id))
                        }}
                        on_activate={|_: u64| {}}
                    />
                </Stack>
            </Frame>
        }
    });
    let mut harness = Harness::new(document);
    harness.frame(Vec::new());
    let edit = edit.get();
    harness.click(harness.rect(edit).center());
    harness.frame(Vec::new());
    assert_eq!(harness.document().focused_node(), Some(edit));

    let reply = harness
        .document()
        .find_test_id("toast.7.action.reply")
        .expect("the toast shows its action");
    for _ in 0..8 {
        if harness.document().focused_node() == Some(reply) {
            break;
        }
        harness.key(Key::Tab, Modifiers::NONE);
        harness.frame(Vec::new());
    }
    assert_eq!(
        harness.document().focused_node(),
        Some(reply),
        "Tab reaches the toast's action"
    );
    harness.key(Key::Enter, Modifiers::NONE);
    harness.frame(Vec::new());
    harness.frame(Vec::new());
    assert!(harness.document().find_test_id("toast.7").is_none());
    assert!(harness.document().find_test_id("toast.8").is_some());
    assert_eq!(
        harness.document().focused_node(),
        Some(edit),
        "the focus goes back where it was before it went into the toast"
    );
}

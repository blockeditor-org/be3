use super::*;
use crate::primitives::Frame;
use crate::styled::*;

const STEPS: usize = 4;
const STEP: f32 = 5.0;

#[test]
fn a_toast_whose_time_runs_out_while_it_is_held_goes_once_let_go() {
    let _root = crate::reactive::RootScope::new();
    let dismissed = create_signal(Vec::<u64>::new());
    let document = build_node(move || {
        let area = NodeRef::new();
        let toasts = create_signal(vec![Toast {
            id: 1,
            message: "Saved".to_owned(),
            ..Toast::default()
        }]);
        view! {
            <Frame @node_ref=&area width={VIEWPORT.x} height={VIEWPORT.y}>
                <Toasts
                    anchor={area}
                    toasts={toasts}
                    on_dismiss={move |id: u64| {
                        dismissed.update(|dismissed| dismissed.push(id));
                        toasts.update(|toasts| toasts.retain(|toast| toast.id != id));
                    }}
                />
            </Frame>
        }
    });
    let mut harness = Harness::new(document);
    harness.settle();
    let card = harness
        .frame(Vec::new())
        .test_id_rect("toast.1")
        .expect("the toast shows");
    let from = pos2(card.left() + 40.0, card.center().y);
    let to = from + vec2(STEPS as f32 * STEP, 0.0);
    harness.frame(vec![
        Input::PointerMoved(from),
        Input::PointerButton {
            pos: from,
            button: PointerButton::Primary,
            pressed: true,
        },
    ]);
    // Slowly, so it is let go at no speed that would throw it.
    for step in 1..=STEPS {
        harness.frame(vec![Input::PointerMoved(from + vec2(step as f32 * STEP, 0.0))]);
    }

    // Its time runs out while the mouse holds it a short way across.
    harness.advance(TOAST_DURATION);
    harness.frame(Vec::new());
    harness.settle();
    assert!(
        dismissed.get_untracked().is_empty(),
        "a held toast stays under the pointer"
    );

    harness.frame(vec![Input::PointerButton {
        pos: to,
        button: PointerButton::Primary,
        pressed: false,
    }]);
    harness.settle();
    assert_eq!(
        dismissed.get_untracked(),
        vec![1],
        "let go short, a toast whose time ran out still goes rather than springing back"
    );
    assert!(harness.frame(Vec::new()).test_id_rect("toast.1").is_none());
}

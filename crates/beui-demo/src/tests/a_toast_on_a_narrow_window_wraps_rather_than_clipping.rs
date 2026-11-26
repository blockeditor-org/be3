use super::*;

#[test]
fn a_toast_on_a_narrow_window_wraps_rather_than_clipping() {
    let size = Vec2::new(240.0, 400.0);
    let document = build(move || {
        let area = NodeRef::new();
        let toasts = vec![Toast {
            id: 1,
            title: "Copied".to_owned(),
            message: "The link to this block is on the clipboard, ready to paste anywhere"
                .to_owned(),
            actions: vec![ToastAction {
                key: "undo".to_owned(),
                label: "Undo".to_owned(),
            }],
            ..Toast::default()
        }];
        view! {
            <Root>
                <Frame @node_ref=&area fill=ColorRole::Background>
                    <Toasts
                        anchor={area}
                        toasts={toasts}
                        on_dismiss={|_: u64| {}}
                        on_action={|_: (u64, String)| {}}
                        on_activate={|_: u64| {}}
                    />
                </Frame>
            </Root>
        }
    });
    let mut test = DocumentTest::new(document, size);
    test.settle();
    let toast = test.rect_of("toast.1");
    assert!(
        toast.left() >= 0.0 && toast.right() <= size.x,
        "the toast fits the window's width, at {toast:?}"
    );
    let dismiss = test.rect_of("toast.1.dismiss");
    assert!(
        dismiss.right() <= toast.right(),
        "its dismiss button shows inside it, at {dismiss:?}"
    );
    test.snapshot("a_toast_on_a_narrow_window");
}

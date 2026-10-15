use beui::reactive::*;
use beui::styled::*;
use beui::unstyled::Edge;

use super::*;

#[test]
fn toasts_on_the_top_edge_stack_in_the_top_corner() {
    let size = Vec2::new(800.0, 500.0);
    let document = build(move || {
        let theme = use_theme();
        let area = NodeRef::new();
        let toasts = vec![
            Toast {
                id: 1,
                message: "Terminal did not start: No such file or directory".to_owned(),
                danger: true,
                ..Toast::default()
            },
            Toast {
                id: 2,
                message: "The last session ended after an error: the host lost track of a screen"
                    .to_owned(),
                danger: true,
                actions: vec![ToastAction {
                    key: "copy-report".to_owned(),
                    label: "Copy report".to_owned(),
                }],
                ..Toast::default()
            },
        ];
        view! {
            <Frame @node_ref=&area fill={theme.background}>
                <Toasts
                    anchor={area}
                    edge=Edge::TopEnd
                    toasts={toasts}
                    on_dismiss={|_: u64| {}}
                    on_action={|_: (u64, String)| {}}
                    on_activate={|_: u64| {}}
                />
            </Frame>
        }
    });
    let mut test = DocumentTest::new(document, size);
    let toast = test.rect_of("toast.1");
    assert!(
        toast.top() < size.y / 4.0 && toast.right() > size.x * 3.0 / 4.0,
        "the toast is in the top-right corner: {toast:?}"
    );
    test.snapshot("a_toast_on_the_top_edge");
}

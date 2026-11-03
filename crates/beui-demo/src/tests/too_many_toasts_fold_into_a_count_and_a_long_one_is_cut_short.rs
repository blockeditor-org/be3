use beui::unstyled::Edge;

use super::*;

#[test]
fn too_many_toasts_fold_into_a_count_and_a_long_one_is_cut_short() {
    let size = Vec2::new(800.0, 600.0);
    let document = build(move || {
        let area = NodeRef::new();
        let toasts = create_signal(
            (1..=6)
                .map(|id| Toast {
                    id,
                    title: format!("Bulk {id}"),
                    message: match id {
                        6 => "word ".repeat(400),
                        _ => "x".to_owned(),
                    },
                    sticky: true,
                    ..Toast::default()
                })
                .collect::<Vec<_>>(),
        );
        view! {
            <Root>
                <Frame @node_ref=&area fill=ColorRole::Background>
                    <Toasts
                        anchor={area}
                        edge=Edge::BottomEnd
                        toasts={toasts}
                        on_dismiss={move |id: u64| {
                            toasts.update(|toasts| toasts.retain(|toast| toast.id != id))
                        }}
                        on_action={|_: (u64, String)| {}}
                        on_activate={|_: u64| {}}
                    />
                </Frame>
            </Root>
        }
    });
    let mut test = DocumentTest::new(document, size);
    for id in 1..=3 {
        assert!(
            !test.shows(&format!("toast.{id}")),
            "toast {id} is folded away"
        );
    }
    for id in 4..=6 {
        assert!(
            test.shows(&format!("toast.{id}")),
            "the newest toasts show: {id}"
        );
    }
    let more = test.rect_of("toasts.more");
    let newest = test.rect_of("toast.6");
    assert!(
        more.bottom() <= test.rect_of("toast.4").top(),
        "the count sits where the folded toasts would: {more:?}"
    );
    assert!(
        newest.height() < 200.0,
        "a 400-word toast is cut to a few lines: {newest:?}"
    );
    assert!(
        newest.bottom() <= size.y,
        "and the stack stays inside its area: {newest:?}"
    );
    test.snapshot("toasts_folded_into_a_count");

    test.click("toast.6.dismiss");
    assert!(
        test.shows("toast.3"),
        "dismissing a toast brings the next one back"
    );
    test.click("toast.5.dismiss");
    test.click("toast.4.dismiss");
    assert!(
        !test.shows("toasts.more"),
        "the count goes once every toast fits"
    );
}

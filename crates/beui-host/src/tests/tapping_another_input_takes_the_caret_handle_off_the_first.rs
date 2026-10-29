use super::*;
use crate::styled::*;

#[test]
fn tapping_another_input_takes_the_caret_handle_off_the_first() {
    let (document, [first, second]) = toolbar_of(|| {
        [
            view! {
                <TextInput value="First" />
            },
            view! {
                <TextInput value="Second" />
            },
        ]
    });
    let mut harness = Harness::new(document);
    harness.frame(Vec::new());
    let tap = |harness: &mut Harness, input: NodeId| {
        let text = harness.rect(
            harness
                .document()
                .api::<unstyled::TextInputApi>(input)
                .text(),
        );
        let pos = pos2(text.right() - 1.0, text.center().y);
        harness.touch(TouchPhase::Start, pos);
        harness.touch(TouchPhase::End, pos);
        harness.frame(Vec::new())
    };
    let handles = |harness: &Harness, output: &crate::native::FrameOutput| -> Vec<NodeId> {
        output
            .main()
            .shapes()
            .iter()
            .filter_map(|shape| match shape {
                crate::native::Shape::Rect { rect, .. }
                    if rect.size() == Vec2::splat(beui_core::rich::HANDLE_RADIUS * 2.0) =>
                {
                    [first, second].into_iter().find(|input| {
                        let field = harness.rect(*input);
                        (field.top()..field.bottom()).contains(&rect.top())
                    })
                }
                _ => None,
            })
            .collect()
    };

    let output = tap(&mut harness, first);
    assert_eq!(
        handles(&harness, &output),
        [first],
        "tapping an input shows its caret handle"
    );
    harness.advance(Duration::from_secs(1));
    tap(&mut harness, second);
    let output = harness.frame(Vec::new());
    assert_eq!(
        handles(&harness, &output),
        [second],
        "only the input tapped last shows a caret handle"
    );
}

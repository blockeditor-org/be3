use super::*;
use beui::reactive::view;
use beui::vec2;
use beui::{TouchId, TouchPhase};

#[test]
fn a_fitted_sheet_pulled_past_its_top_carries_its_surface_to_the_bottom() {
    let document = build(move || {
        view! {
            <Root>
                <ModalSheet open=true fit=true on_close={|| ()}>
                    <List spacing=Space::None>
                        <ForEach value={(1..=3).collect::<Vec<usize>>()}>|row: usize|
                            <ActionRow label={format!("Row {row}")} />
                        </ForEach>
                    </List>
                </ModalSheet>
            </Root>
        }
    });
    let mut test = DocumentTest::new(document, vec2(400.0, 600.0));
    let touch = |test: &mut DocumentTest, phase, pos| {
        test.frame(vec![Input::Touch {
            id: TouchId {
                device: 1,
                finger: 0,
            },
            phase,
            pos,
            force: None,
        }]);
    };
    test.settle();
    let fitted = test.rect_of("sheet.handle").top();
    let from = pos2(200.0, fitted + 60.0);
    touch(&mut test, TouchPhase::Start, from);
    for step in 1..=20 {
        touch(
            &mut test,
            TouchPhase::Move,
            from - vec2(0.0, step as f32 * 10.0),
        );
    }
    assert!(
        test.rect_of("sheet.handle").top() < fitted - 20.0,
        "pulled past where it fits, the sheet rises a little way"
    );
    // Its surface reaches the bottom edge below its rows, rather than the scrim showing
    // through a gap under them.
    test.snapshot("a_fitted_sheet_pulled_past_its_top");
    touch(&mut test, TouchPhase::End, from - vec2(0.0, 200.0));
    test.settle();
    assert_eq!(test.rect_of("sheet.handle").top(), fitted);
}

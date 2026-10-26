use super::*;
use beui::vec2;
use beui::{TouchId, TouchPhase};
use beui_macros::view;

#[test]
fn a_long_sheet_fades_its_content_and_bounces_as_a_panel() {
    let document = build(move || {
        view! {
            <Root>
                <ModalSheet open=true rest=0.5 on_close={|| ()}>
                    <List spacing=Space::None>
                        <ForEach keys={(1..=60).collect::<Vec<usize>>()}>|row: usize|
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
    let from = test.rect_of("sheet.outside").center();
    touch(&mut test, TouchPhase::Start, from);
    touch(&mut test, TouchPhase::Move, from - vec2(0.0, 230.0));
    test.snapshot("half_open_sheet_expands_from_the_backdrop");
    touch(&mut test, TouchPhase::End, from - vec2(0.0, 230.0));
    test.settle();
    assert_eq!(test.rect_of("sheet.handle").top(), 60.0);
    test.snapshot("sheet_fades_at_the_start");
    test.scroll(pos2(200.0, 300.0), vec2(0.0, 300.0));
    test.settle();
    test.snapshot("sheet_fades_in_the_middle");
    let top = test.rect_of("sheet.handle").top();
    touch(&mut test, TouchPhase::Start, pos2(200.0, 200.0));
    touch(&mut test, TouchPhase::Move, pos2(200.0, 260.0));
    touch(&mut test, TouchPhase::Move, pos2(200.0, 320.0));
    touch(&mut test, TouchPhase::End, pos2(200.0, 320.0));
    let mut bounced = false;
    for _ in 0..120 {
        test.frame(Vec::new());
        if test.rect_of("sheet.handle").top() > top + 4.0 {
            bounced = true;
            test.snapshot("sheet_bounces_at_the_start");
            break;
        }
    }
    assert!(bounced);
    test.settle();
    test.scroll(pos2(200.0, 300.0), vec2(0.0, 10000.0));
    test.settle();
    test.snapshot("sheet_fades_at_the_end");
    let from = test.rect_of("sheet.handle").center();
    touch(&mut test, TouchPhase::Start, from);
    touch(&mut test, TouchPhase::Move, from + vec2(0.0, 160.0));
    test.snapshot("scrolled_sheet_dragged_by_the_handle");
    touch(&mut test, TouchPhase::End, from + vec2(0.0, 160.0));
    test.settle();
    let from = test.rect_of("sheet.outside").center();
    touch(&mut test, TouchPhase::Start, from);
    touch(&mut test, TouchPhase::Move, from + vec2(0.0, 120.0));
    test.snapshot("sheet_dragged_from_above");
    touch(&mut test, TouchPhase::Cancel, from + vec2(0.0, 120.0));
}

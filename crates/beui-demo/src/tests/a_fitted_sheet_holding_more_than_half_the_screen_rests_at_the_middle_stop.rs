use super::*;
use beui::reactive::view;
use beui::vec2;

#[test]
fn a_fitted_sheet_holding_more_than_half_the_screen_rests_at_the_middle_stop() {
    let document = build(move || {
        view! {
            <Root>
                <ModalSheet open=true fit=true on_close={|| ()}>
                    <List spacing=Space::None>
                        <ForEach value={(1..=20).collect::<Vec<usize>>()}>|row: usize|
                            <ActionRow label={format!("Row {row}")} />
                        </ForEach>
                    </List>
                </ModalSheet>
            </Root>
        }
    });
    let mut test = DocumentTest::new(document, vec2(400.0, 600.0));
    test.settle();
    assert_eq!(
        test.rect_of("sheet.handle").top(),
        300.0,
        "it rises no higher than the middle stop, and what it holds scrolls under its grip"
    );
    test.snapshot("a_fitted_sheet_at_the_middle_stop");
}

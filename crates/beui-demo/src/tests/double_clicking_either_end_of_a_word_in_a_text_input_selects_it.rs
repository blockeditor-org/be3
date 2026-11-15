use std::time::Duration;

use beui_host::native::testing::Target;

use super::*;

#[test]
fn double_clicking_either_end_of_a_word_in_a_text_input_selects_it() {
    let document = build(|| {
        view! {
            <Root>
                <Frame fill=ColorRole::Background padding=Space::Lg>
                    <TextInput @test_id="test.field" value="Weekend trip" />
                </Frame>
            </Root>
        }
    });
    let mut test = DocumentTest::new(document, Vec2::new(320.0, 72.0));
    let input = test.document().find_test_id("test.field").unwrap();
    let text = test
        .document()
        .texts_within(input)
        .into_iter()
        .find(|text| test.document().text(*text) == "Weekend trip")
        .expect("the field shows its value");
    // Just after "Weekend", before the space, and past the end of "trip".
    for (index, beyond, selected) in [(7, 0.0, 0..7), (12, 24.0, 8..12)] {
        let caret = test.document().text_caret_rect(text, index, 2.0).unwrap();
        test.double_click(Target::point(pos2(caret.center().x + beyond, caret.center().y)));
        test.leave();
        let selection = test
            .document()
            .api::<beui::unstyled::TextInputApi>(input)
            .selection();
        assert_eq!(selection, [selected]);
        test.record();
        test.advance(Duration::from_secs(1));
    }
    test.snapshot("double_clicking_either_end_of_a_word_in_a_text_input_selects_it");
}

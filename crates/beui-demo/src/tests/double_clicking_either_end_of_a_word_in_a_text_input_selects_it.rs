use std::time::Duration;

use beui_host::native::testing::Target;

use super::*;

#[test]
#[allow(clippy::single_range_in_vec_init)] // a list of ranges, which here holds one
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
    let double_click = |test: &mut DocumentTest, index: usize, beyond: f32| {
        let caret = test.document().text_caret_rect(text, index, 2.0).unwrap();
        test.double_click(Target::point(pos2(
            caret.center().x + beyond,
            caret.center().y,
        )));
        test.leave();
        test.document()
            .api::<beui::unstyled::TextInputApi>(input)
            .selection()
    };
    // Just after "Weekend", before the space, the word before it.
    assert_eq!(double_click(&mut test, 7, 0.0), [0..7]);
    test.record();
    // A double click a second later is a new gesture. Past the end, the last word.
    test.advance(Duration::from_secs(1));
    assert_eq!(double_click(&mut test, 12, 24.0), [8..12]);
    test.record();
    test.snapshot("double_clicking_either_end_of_a_word_in_a_text_input_selects_it");
}

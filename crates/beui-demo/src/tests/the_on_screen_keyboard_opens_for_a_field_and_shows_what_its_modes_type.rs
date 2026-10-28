use beui::keyboard::styled::{BAR_HEIGHT, PADDING, key_center};
use beui::keyboard::unstyled::{Layer, Modifier, Value};
use beui::vec2;

use super::*;

const SIZE: Vec2 = Vec2::new(420.0, 640.0);

fn field() -> DocumentTest {
    let document = build(move || {
        view! {
            <Root>
                <Frame fill=ColorRole::Background padding=Space::Xl>
                    <TextInput
                        @test_id={"test.field"}
                        value=String::new()
                        placeholder="Type here"
                    />
                </Frame>
            </Root>
        }
    });
    DocumentTest::new(document, SIZE)
}

fn key(test: &DocumentTest, layer: Layer, value: Value) -> Pos2 {
    let keyboard = test
        .context()
        .keyboard_rect()
        .expect("the keyboard is open");
    key_center(keyboard, layer, value).expect("the layer has that key")
}

fn swipe(test: &mut DocumentTest, layer: Layer, value: Value, by: Vec2) {
    let from = key(test, layer, value);
    test.drag(from, from + by);
    test.frame(Vec::new());
}

fn value_of(test: &DocumentTest) -> String {
    let field = test
        .document()
        .find_test_id("test.field")
        .expect("the field");
    test.document().api::<TextInputApi>(field).value()
}

#[test]
fn the_on_screen_keyboard_opens_for_a_field_and_shows_what_its_modes_type() {
    let mut test = field();
    test.context().set_keyboard_attached(false);
    test.frame(Vec::new());
    assert!(
        test.context().keyboard_rect().is_none(),
        "nothing asks for it yet"
    );

    test.click("test.field");
    test.frame(Vec::new());
    let keyboard = test
        .context()
        .keyboard_rect()
        .expect("tapping a field opens the keyboard when no keyboard is attached");
    assert_eq!(
        keyboard.bottom(),
        SIZE.y,
        "the keyboard sits along the bottom"
    );
    test.snapshot("software_keyboard");

    let pressed = key(&test, Layer::Text, Value::Modifier(Modifier::Shift));

    test.click(pressed);
    test.frame(Vec::new());
    test.snapshot("software_keyboard_shifted");
    for letter in ['h', 'i'] {
        let pressed = key(&test, Layer::Text, Value::Char(letter));
        test.click(pressed);
    }
    test.frame(Vec::new());
    assert_eq!(value_of(&test), "Hi", "shift lasts for one key");

    swipe(&mut test, Layer::Text, Value::Char('s'), vec2(16.0, 16.0));
    test.snapshot("software_keyboard_superscript");
    swipe(&mut test, Layer::Text, Value::Char('w'), vec2(16.0, -16.0));
    test.frame(Vec::new());
    assert_eq!(
        value_of(&test),
        "Hi²",
        "superscript turns the next key into its superscript"
    );

    let pressed = key(&test, Layer::Text, Value::Compose);

    test.click(pressed);
    let pressed = key(&test, Layer::Text, Value::Char('a'));
    test.click(pressed);
    test.frame(Vec::new());
    test.snapshot("software_keyboard_composing");
    let pressed = key(&test, Layer::Text, Value::Char('e'));
    test.click(pressed);
    test.frame(Vec::new());
    assert_eq!(
        value_of(&test),
        "Hi²æ",
        "the key that showed æ while composing types it"
    );

    swipe(
        &mut test,
        Layer::Text,
        Value::Modifier(Modifier::Fn),
        vec2(-16.0, 16.0),
    );
    test.snapshot("software_keyboard_emoji");

    let close = pos2(
        keyboard.right() - PADDING.px() - BAR_HEIGHT / 2.0,
        keyboard.top() + PADDING.px() + BAR_HEIGHT / 2.0,
    );
    test.click(close);
    test.frame(Vec::new());
    assert!(
        test.context().keyboard_rect().is_none(),
        "its close button closes it"
    );

    test.click("test.field");
    test.frame(Vec::new());
    assert!(
        test.context().keyboard_rect().is_some(),
        "tapping the field again opens it again"
    );
}

use super::*;
use beui::Key;

#[test]
fn typing_a_fahrenheit_temperature_moves_the_celsius_one() {
    let mut test = alone(WIDE, Page::Inputs);
    test.frame(Vec::new());
    reveal(&mut test, "demo.temperature.fahrenheit");
    let root = test.document().root().expect("the demo built a root");
    assert_eq!(showing(test.document(), root, "68"), 1);

    test.click("demo.temperature.fahrenheit");
    test.frame(Vec::new());
    test.frame(vec![Input::Text("212".to_owned())]);
    press_key(&mut test, Key::Enter);

    assert_eq!(
        showing(test.document(), root, "100"),
        1,
        "the celsius field reads the signal the fahrenheit one wrote through"
    );
    test.snapshot("two_number_inputs_bound_to_one_temperature");
}

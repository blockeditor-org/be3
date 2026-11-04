use super::*;

#[test]
fn a_new_window_fills_the_oldest_placeholder_of_its_program() {
    let mut before = Fixture::new();
    before.settle();
    before.show_windows(&[foot(1), notes(2), foot(3)]);
    let [one, two, three] = [1, 2, 3].map(|id| before.tab_of(id));
    let mut resumed = Fixture::resumed(before.saved());
    let places = [one, three].map(|key| resumed.placeholder_rect(key).expect("a placeholder"));

    let calculator = program_window(40, "calculator", "Calculator", WindowOrigin::default());
    resumed.show_windows(&[calculator.clone(), foot(41)]);
    assert!(
        resumed.placeholder_rect(one).is_none(),
        "a terminal started by itself fills the oldest terminal's placeholder"
    );
    assert_eq!(
        resumed.window_rect(41).map(|rect| rect.min),
        Some(places[0].min)
    );
    assert!(resumed.placeholder_rect(three).is_some());
    assert!(
        resumed.placeholder_rect(two).is_some(),
        "another program's placeholder waits for its own"
    );

    resumed.show_windows(&[calculator.clone(), foot(41), foot(42)]);
    assert!(resumed.placeholder_rect(three).is_none());
    assert_eq!(
        resumed.window_rect(42).map(|rect| rect.min),
        Some(places[1].min)
    );

    resumed.show_windows(&[calculator, foot(42)]);
    assert!(
        resumed.placeholder_rect(one).is_none() && resumed.placeholder_rect(three).is_none(),
        "a window that filled a placeholder and closes leaves no placeholder behind"
    );
    assert_eq!(
        resumed.window_rect(42).map(|rect| rect.min),
        Some(places[1].min),
        "and the other terminal stays where it is"
    );
}

use super::*;

#[test]
fn typing_leaves_the_panel_in_four_greys() {
    let at = Duration::from_millis;
    let mut pace = Pace::default();
    pace.input(Input::Press(Held::Key(30)), at(0));
    pace.input(Input::Release(Held::Key(30)), at(50));
    pace.drew(at(60));
    assert_eq!(pace.step(at(60)), None);
    assert_eq!(pace.settles_at(), None);
}

use super::*;
use crate::reactive::*;
use crate::styled::*;

#[test]
fn a_scrub_released_off_its_face_is_kept_once() {
    let _root = crate::reactive::RootScope::new();
    let drags = create_signal(Vec::new());
    let (document, [input]) = toolbar_of(|| {
        [view! {
            <NumberInput
                value={4.0}
                min=0.0
                max=100.0
                on_scrub={move |change| drags.update(|drags| drags.push(change))}
            />
        }]
    });
    let mut harness = Harness::new(document);
    harness.frame(Vec::new());
    let middle = harness.center(input);
    let away = middle + Vec2::new(60.0, 120.0);

    harness.down(middle, PointerButton::Primary);
    harness.frame(vec![Input::PointerMoved(middle + Vec2::new(30.0, 0.0))]);
    harness.frame(vec![Input::PointerMoved(away)]);
    harness.up(away, PointerButton::Primary);
    harness.frame(Vec::new());

    assert_eq!(
        drags.get_untracked(),
        vec![ValueDrag::Started, ValueDrag::Kept],
        "a release away from the face, which is no click, still ends the scrub, once"
    );
}

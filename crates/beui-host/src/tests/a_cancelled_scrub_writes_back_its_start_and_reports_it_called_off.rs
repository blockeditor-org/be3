use super::*;
use crate::reactive::*;
use crate::styled::*;

#[test]
fn a_cancelled_scrub_writes_back_its_start_and_reports_it_called_off() {
    let _root = crate::reactive::RootScope::new();
    let changes = create_signal(Vec::new());
    let drags = create_signal(Vec::new());
    let (document, [input]) = toolbar_of(|| {
        [view! {
            <NumberInput
                value={Binding::new(4.0, move |next| changes.update(|changes| changes.push(next)))}
                min=0.0
                max=100.0
                drag={NumberDrag::Linear { speed: 0.5 }}
                on_scrub={move |change| drags.update(|drags| drags.push(change))}
            />
        }]
    });
    let mut harness = Harness::new(document);
    harness.frame(Vec::new());
    let middle = harness.center(input);

    harness.down(middle, PointerButton::Primary);
    harness.frame(vec![Input::PointerMoved(middle + Vec2::new(20.0, 0.0))]);
    assert_eq!(drags.get_untracked(), vec![ValueDrag::Started]);
    harness.frame(vec![Input::PointersCancelled]);

    assert_eq!(
        changes.with_untracked(|changes| changes.last().copied()),
        Some(4.0),
        "a cancelled scrub writes back where it started: {:?}",
        changes.get_untracked()
    );
    assert_eq!(
        drags.get_untracked(),
        vec![ValueDrag::Started, ValueDrag::Cancelled],
        "and says it was called off, once"
    );
}

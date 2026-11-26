use super::*;
use crate::primitives::List;
use crate::reactive::*;
use crate::styled::*;

#[test]
fn a_slider_removed_mid_drag_reports_the_drag_called_off() {
    let _root = crate::reactive::RootScope::new();
    let drags = create_signal(Vec::new());
    let shown = create_signal(true);
    let (document, [holder]) = toolbar_of(|| {
        [view! {
            <List spacing=0.0>
                <Show value={shown}>
                    <Slider
                        value={0.2}
                        on_drag_change={move |change| drags.update(|drags| drags.push(change))}
                    />
                </Show>
            </List>
        }]
    });
    let mut harness = Harness::new(document);
    harness.frame(Vec::new());
    let track = harness.rect(holder);
    let at = pos2(track.left() + 10.0, track.center().y);

    harness.down(at, PointerButton::Primary);
    harness.frame(vec![Input::PointerMoved(at + Vec2::new(30.0, 0.0))]);
    assert_eq!(drags.get_untracked(), vec![ValueDrag::Started]);
    shown.set(false);
    harness.frame(vec![Input::PointerMoved(at + Vec2::new(40.0, 0.0))]);
    harness.up(at + Vec2::new(40.0, 0.0), PointerButton::Primary);
    harness.frame(Vec::new());

    assert_eq!(
        drags.get_untracked(),
        vec![ValueDrag::Started, ValueDrag::Cancelled],
        "a control that goes mid-drag ends the drag, so its owner is never left holding it"
    );
}

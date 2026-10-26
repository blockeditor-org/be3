use super::*;
use crate::input::SecondaryDrag;
use crate::primitives::*;
use crate::reactive::*;

#[test]
fn right_dragging_over_a_click_catcher_reports_a_secondary_drag() {
    let _root = crate::reactive::RootScope::new();
    let drags = create_signal::<Vec<SecondaryDrag>>(Vec::new());
    let document = build_node(move || {
        view! {
            <Interactive on_secondary_drag={move |drag| drags.update(|drags| drags.push(drag))}>
                <Frame width=200.0 height=200.0 />
            </Interactive>
        }
    });
    let mut harness = Harness::new(document);
    harness.frame(Vec::new());
    let button = |pos, pressed| Input::PointerButton {
        pos,
        button: PointerButton::Secondary,
        pressed,
    };

    harness.frame(vec![
        Input::Modifiers(Modifiers::SHIFT),
        Input::PointerMoved(pos2(20.0, 30.0)),
    ]);
    harness.frame(vec![button(pos2(20.0, 30.0), true)]);
    harness.frame(vec![Input::PointerMoved(pos2(120.0, 90.0))]);
    harness.frame(vec![button(pos2(150.0, 100.0), false)]);
    harness.frame(Vec::new());

    let drags = drags.get_untracked();
    let seen: Vec<(Pos2, Pos2, bool, bool)> = drags
        .iter()
        .map(|drag| (drag.from, drag.pos, drag.started, drag.ended))
        .collect();
    assert_eq!(
        seen,
        [
            (pos2(20.0, 30.0), pos2(20.0, 30.0), true, false),
            (pos2(20.0, 30.0), pos2(120.0, 90.0), false, false),
            (pos2(20.0, 30.0), pos2(150.0, 100.0), false, true),
        ]
    );
    assert!(drags.iter().all(|drag| drag.modifiers == Modifiers::SHIFT));
}

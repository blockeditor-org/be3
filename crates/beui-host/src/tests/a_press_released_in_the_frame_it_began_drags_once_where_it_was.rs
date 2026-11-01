use super::*;
use crate::primitives::*;
use crate::reactive::*;
use beui::PointerPress;

#[test]
fn a_press_released_in_the_frame_it_began_drags_once_where_it_was() {
    let _root = crate::reactive::RootScope::new();
    let drags = create_signal::<Vec<Pos2>>(Vec::new());
    let document = build_node(move || {
        view! {
            <Interactive
                on_drag={move |press: PointerPress| drags.update(|drags| drags.push(press.local))}
            >
                <Frame width=300.0 height=300.0 />
            </Interactive>
        }
    });
    let mut harness = Harness::new(document);
    harness.frame(Vec::new());
    let at = pos2(40.0, 60.0);
    let button = |pressed| Input::PointerButton {
        pos: at,
        button: PointerButton::Primary,
        pressed,
    };

    harness.frame(vec![Input::PointerMoved(at), button(true), button(false)]);

    assert_eq!(
        drags.get_untracked(),
        [at],
        "a quick click drags where it was pressed, as a slower one does at the end of its first frame"
    );
}

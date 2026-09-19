use super::*;
use crate::reactive::{
    List, Show, WriteSignal, create_memo, create_signal, view, with_reactive_scope,
};

const FIRST: f32 = 60.0;
const SECOND: f32 = 90.0;

#[test]
fn switching_a_show_damages_both_panels() {
    let taken: Rc<Cell<Option<WriteSignal<bool>>>> = Rc::new(Cell::new(None));
    let sink = taken.clone();
    let document = build(move || {
        let (second, set_second) = create_signal(false);
        sink.set(Some(set_second));
        let showing = second.clone();
        let first = create_memo(move || !showing.get());
        let second = create_memo(move || second.get());
        view! {
            <List spacing=0.0>
                <Show condition={first}>
                    <Frame height={FIRST} color=Color32::WHITE radius=0 />
                </Show>
                <Show condition={second}>
                    <Frame height={SECOND} color={Color32::from_gray(40)} radius=0 />
                </Show>
            </List>
        }
    });
    let set_second = taken.take().expect("the view published its signal");
    let mut harness = Harness::new(document);
    harness.frame(Vec::new());

    with_reactive_scope(harness.document_mut(), move || set_second.set(true));
    let damage = harness
        .frame(Vec::new())
        .damage()
        .expect("swapping the shown panel repaints");

    assert_eq!(
        damage,
        Rect::from_min_max(pos2(0.0, 0.0), pos2(VIEWPORT.x, VIEWPORT.y)),
        "taking a panel out of the column and putting another in damages the column that lays \
         them out, which covers the panel that went and the one that came"
    );
}

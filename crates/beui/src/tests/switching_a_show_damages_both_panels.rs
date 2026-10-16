use super::*;
use crate::primitives::*;
use crate::reactive::*;

const FIRST: f32 = 60.0;
const SECOND: f32 = 90.0;

#[test]
fn switching_a_show_damages_both_panels() {
    let _root = crate::reactive::RootScope::new();
    let (taken, set_taken) = create_signal::<Option<WriteSignal<bool>>>(None);
    let document = build(move || {
        let (second, set_second) = create_signal(false);
        set_taken.update(|taken| *taken = Some(set_second));
        let first = create_memo(move || !second.get());
        let second = create_memo(move || second.get());
        view! {
            <List spacing=0.0>
                <Show condition={first}>
                    <Frame height=FIRST fill=Color32::WHITE radius=0.0 />
                </Show>
                <Show condition={second}>
                    <Frame height=SECOND fill={Color32::from_gray(40)} radius=0.0 />
                </Show>
            </List>
        }
    });
    let set_second = taken
        .get_untracked()
        .expect("the view published its signal");
    let mut harness = Harness::new(document);
    harness.frame(Vec::new());

    set_second.set(true);
    let damage = harness
        .frame(Vec::new())
        .damage()
        .expect("swapping the shown panel repaints");

    assert_eq!(
        damage,
        Rect::from_min_max(pos2(0.0, 0.0), pos2(VIEWPORT.x, SECOND)),
        "taking a panel out of the column and putting another in damages what the column \
         painted before and after, which covers the panel that went and the one that came"
    );
}

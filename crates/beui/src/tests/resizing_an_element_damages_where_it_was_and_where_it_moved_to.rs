use super::*;
use crate::primitives::*;
use crate::reactive::*;

const SHORT: f32 = 60.0;
const TALL: f32 = 120.0;
const BOTTOM: f32 = 40.0;

#[test]
fn resizing_an_element_damages_where_it_was_and_where_it_moved_to() {
    let _root = crate::reactive::RootScope::new();
    let bottom = NodeRef::new();
    let taken = create_signal::<Option<Signal<bool>>>(None);
    let document = build_node(move || {
        let tall = create_signal(false);
        taken.update(|taken| *taken = Some(tall));
        let height = create_memo(move || if tall.get() { TALL } else { SHORT });
        view! {
            <List spacing=0.0>
                <Frame height=height fill=Color32::WHITE radius=0.0 />
                <Frame @node_ref=&bottom height=BOTTOM fill={Color32::from_gray(40)} radius=0.0 />
            </List>
        }
    });
    let bottom = bottom.get();
    let tall = taken
        .get_untracked()
        .expect("the view published its signal");
    let mut harness = Harness::new(document);
    harness.frame(Vec::new());
    let was = harness.rect(bottom);

    tall.set(true);
    let damage = harness
        .frame(Vec::new())
        .main()
        .damage()
        .expect("resizing repaints");
    let moved = harness.rect(bottom);

    assert_eq!(was.top(), SHORT);
    assert_eq!(moved.top(), TALL);
    assert_eq!(
        damage,
        Rect::from_min_max(pos2(0.0, 0.0), pos2(VIEWPORT.x, TALL + BOTTOM))
    );
}

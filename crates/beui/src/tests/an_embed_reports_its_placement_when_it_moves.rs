use super::*;
use crate::primitives::*;
use crate::reactive::*;

#[test]
fn an_embed_reports_its_placement_when_it_moves() {
    let _root = crate::reactive::RootScope::new();
    let placed = create_signal(Vec::new());
    let slot = EmbedSlot::new();
    slot.on_place(move |placement| {
        placed.update(|placed| placed.push(placement.map(|at| at.rect)))
    });
    let moved = create_signal(None);
    let document = build_node(move || {
        let x = create_signal(10.0);
        moved.update(|moved| *moved = Some(x));
        view! {
            <Canvas>
                <CanvasItem x={x} y=20.0 width=30.0 height=40.0>
                    <Embed slot={slot} />
                </CanvasItem>
            </Canvas>
        }
    });
    let mut harness = Harness::new(document);
    harness.frame(Vec::new());
    harness.frame(Vec::new());
    assert_eq!(
        placed.get_untracked(),
        [Some(Rect::from_min_size(
            pos2(10.0, 20.0),
            vec2(30.0, 40.0)
        ))],
        "a placement that did not change is not reported again"
    );

    let x = moved.get_untracked().expect("the view was built");
    x.set(60.0);
    harness.frame(Vec::new());
    assert_eq!(
        placed.get_untracked().last().copied().flatten(),
        Some(Rect::from_min_size(pos2(60.0, 20.0), vec2(30.0, 40.0)))
    );
}

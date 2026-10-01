use std::cell::RefCell;
use std::rc::Rc;

use super::*;
use crate::reactive::{
    Canvas, CanvasItem, Embed, EmbedSlot, build, create_signal, view, with_reactive_scope,
};

#[test]
fn an_embed_reports_its_placement_when_it_moves() {
    let placed = Rc::new(RefCell::new(Vec::new()));
    let slot = EmbedSlot::new();
    slot.on_place({
        let placed = Rc::clone(&placed);
        move |placement| placed.borrow_mut().push(placement.map(|at| at.rect))
    });
    let moved = Rc::new(RefCell::new(None));
    let document = build({
        let slot = slot.clone();
        let moved = Rc::clone(&moved);
        move || {
            let (x, set_x) = create_signal(10.0);
            *moved.borrow_mut() = Some(set_x);
            view! {
                <Canvas>
                    <CanvasItem x={x} y=20.0 width=30.0 height=40.0>
                        <Embed slot={slot.clone()} />
                    </CanvasItem>
                </Canvas>
            }
        }
    });
    let mut harness = Harness::new(document);
    harness.frame(Vec::new());
    harness.frame(Vec::new());
    assert_eq!(
        *placed.borrow(),
        [Some(Rect::from_min_size(pos2(10.0, 20.0), vec2(30.0, 40.0)))],
        "a placement that did not change is not reported again"
    );

    let set_x = moved.borrow_mut().take().expect("the view was built");
    with_reactive_scope(harness.document_mut(), move || set_x.set(60.0));
    harness.frame(Vec::new());
    assert_eq!(
        placed.borrow().last().copied().flatten(),
        Some(Rect::from_min_size(pos2(60.0, 20.0), vec2(30.0, 40.0)))
    );
}

use super::*;
use crate::reactive::{Drawing, view};

const PANEL_HEIGHT: f32 = 200.0;

#[test]
fn a_deadline_repaint_only_damages_the_element_that_asked_for_it() {
    let (panel, drawing) = (NodeRef::new(), NodeRef::new());
    let mut document = build({
        let (panel, drawing) = (panel.clone(), drawing.clone());
        move || {
            view! {
                <List spacing=0.0>
                    <Frame @node_ref=&panel height={PANEL_HEIGHT} color=Color32::WHITE radius=0 />
                    <Frame width=40.0 height=20.0>
                        <Drawing @node_ref=&drawing draw={alternating()} />
                    </Frame>
                </List>
            }
        }
    });
    let (panel, drawing) = (panel.get(), drawing.get());
    let (_, paints) = counted(&mut document, panel);
    let mut harness = Harness::new(document);
    harness.frame(Vec::new());
    let settled = paints.get();

    harness
        .document
        .paint_cache
        .get_mut()
        .expire_deadlines(Instant::now());
    let damage = harness
        .frame(Vec::new())
        .damage()
        .expect("the drawing repaints on its deadline");

    assert_eq!(
        paints.get(),
        settled,
        "a deadline repaint must not reach past the element that asked for it"
    );
    assert!(damage.intersects(harness.rect(drawing)));
    assert!(!damage.intersects(harness.rect(panel)));
}

fn alternating() -> crate::reactive::Draw {
    let lit = Rc::new(Cell::new(false));
    Rc::new(move |painter: &crate::painter::Painter, rect: Rect| {
        lit.set(!lit.get());
        let color = match lit.get() {
            true => Color32::WHITE,
            false => Color32::BLACK,
        };
        painter.rect_filled(rect, 0.0, color);
        painter.ctx().request_repaint_after(BLINK);
    })
}

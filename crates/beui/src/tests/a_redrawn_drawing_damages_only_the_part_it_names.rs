use super::*;
use crate::damage::Region;
use crate::drawing::Drawing;
use crate::reactive::{Drawing, draw_gpu, view};
use crate::renderer::{Draw, DrawAt};

struct Blank;

impl Draw for Blank {
    fn paint(&self, _pass: &mut wgpu::RenderPass<'_>, _at: DrawAt) {}
}

#[test]
fn a_redrawn_drawing_damages_only_the_part_it_names() {
    let viewport = NodeRef::new();
    let document = build({
        let viewport = viewport.clone();
        move || {
            view! {
                <List spacing=0.0>
                    <Frame height=50.0 />
                    <Frame width=120.0 height=100.0>
                        <Drawing @node_ref=&viewport draw={draw_gpu(None)} />
                    </Frame>
                </List>
            }
        }
    });
    let viewport = viewport.get();
    let mut harness = Harness::new(document);
    let drawing = crate::renderer::drawing(Blank);
    let show = |drawing: Drawing, harness: &mut Harness| {
        harness
            .document_mut()
            .set_drawing(viewport, draw_gpu(Some(drawing)));
    };
    show(drawing.clone(), &mut harness);
    harness.frame(Vec::new());
    let origin = harness.rect(viewport).min.to_vec2();
    assert_ne!(origin, Vec2::ZERO);

    let changed = Rect::from_min_size(pos2(10.0, 20.0), vec2(30.0, 40.0));
    let redrawn = drawing.redrawn(Region::from(changed));
    show(redrawn.clone(), &mut harness);
    let damage = harness
        .frame(Vec::new())
        .damage()
        .expect("a redrawn drawing damages what it names");
    assert_eq!(damage, changed.translate(origin));

    let first = Rect::from_min_size(pos2(0.0, 0.0), vec2(5.0, 5.0));
    let second = Rect::from_min_size(pos2(50.0, 50.0), vec2(5.0, 5.0));
    show(
        redrawn
            .redrawn(Region::from(first))
            .redrawn(Region::from(second)),
        &mut harness,
    );
    let damage = harness
        .frame(Vec::new())
        .damage()
        .expect("both redraws since the last paint are damaged");
    assert_eq!(
        damage,
        first.translate(origin).union(second.translate(origin))
    );

    show(crate::renderer::drawing(Blank), &mut harness);
    let damage = harness
        .frame(Vec::new())
        .damage()
        .expect("a different drawing damages its whole rectangle");
    assert_eq!(damage, harness.rect(viewport));
}

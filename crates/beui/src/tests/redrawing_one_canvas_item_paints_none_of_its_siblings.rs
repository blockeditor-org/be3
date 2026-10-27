use super::*;
use crate::primitives::*;
use crate::reactive::*;

fn painted_on_redraw(count: usize) -> (usize, usize) {
    let _root = crate::reactive::RootScope::new();
    let view = create_signal(Some(CanvasView::new(pos2(0.0, 0.0), 1.0)));
    let color = create_signal(Color32::from_rgb(200, 0, 0));
    let ids: Vec<usize> = (0..count).collect();
    let document = build_node(move || {
        let draw = create_signal::<Draw>(std::rc::Rc::new(|_, _| {}));
        create_effect(move || {
            let color = color.get();
            draw.set_unconditionally(std::rc::Rc::new(move |painter: &Painter, rect: Rect| {
                painter.rect_filled(rect, 0.0, color)
            }));
        });
        view! {
            <Canvas view={view}>
                <ForEach keys={ids.clone()}>|index: usize|
                    <CanvasItem
                        x={(index % 10) as f32 * 30.0}
                        y={(index / 10) as f32 * 30.0}
                        width=20.0
                        height=20.0
                        scales={index.is_multiple_of(2)}
                    >
                        <Frame fill=Color32::WHITE radius=0.0>
                            <Text>"item"</Text>
                        </Frame>
                    </CanvasItem>
                </ForEach>
                <CanvasItem x=0.0 y=0.0 width=300.0 height=300.0>
                    <Drawing draw={draw} />
                </CanvasItem>
            </Canvas>
        }
    });
    let mut harness = Harness::new(document);
    harness.frame(Vec::new());
    harness.frame(Vec::new());
    color.set(Color32::from_rgb(0, 0, 200));
    harness.frame(Vec::new());
    let work = harness.document().performance().latest.work;
    (work.painted_nodes, work.replayed_nodes)
}

#[test]
fn redrawing_one_canvas_item_paints_none_of_its_siblings() {
    let (few, few_replayed) = painted_on_redraw(4);
    let (many, many_replayed) = painted_on_redraw(80);
    assert_eq!(
        few, many,
        "redrawing one canvas item painted more nodes beside 80 items than beside 4"
    );
    assert_eq!(
        few_replayed, many_replayed,
        "redrawing one canvas item visited more nodes beside 80 items than beside 4"
    );
}

use super::*;
use crate::primitives::*;
use crate::reactive::*;
use crate::unstyled::{TextArea, TextAreaState};
use std::sync::Arc;
use text_editor_core::schema::basic::Basic;
use text_editor_core::{Document, TextBuffer};

fn tallest_glyph(output: &FrameOutput) -> u32 {
    output
        .main()
        .shapes()
        .iter()
        .filter_map(|shape| match shape {
            crate::painter::Shape::Text { galley, .. } => Some(galley.clone()),
            _ => None,
        })
        .flat_map(|galley| {
            galley
                .glyphs()
                .iter()
                .filter_map(|glyph| glyph.image().map(|image| image.height))
                .collect::<Vec<_>>()
        })
        .max()
        .unwrap_or(0)
}

#[test]
fn a_text_area_in_a_scaled_canvas_item_is_not_laid_out_again_by_a_pan_or_zoom() {
    let _root = crate::reactive::RootScope::new();
    let view = create_signal(Some(CanvasView::new(pos2(0.0, 0.0), 1.0)));
    let document = build_node(move || {
        let text = Arc::new(TextBuffer::rich("Hello there", &[])) as Arc<dyn Document>;
        let state = TextAreaState::with_schema(text, Arc::new(Basic));
        view! {
            <Canvas view={view}>
                <CanvasItem x=10.0 y=10.0 width=120.0 height=40.0 scales=true>
                    <TextArea
                        colors={area_colors()}
                        look={area_look()}
                        font_size=AREA_FONT_SIZE
                        padding=AREA_PADDING
                        state
                        line_numbers=false
                        disabled=true
                    />
                </CanvasItem>
            </Canvas>
        }
    });
    let mut harness = Harness::new(document);
    harness.frame(Vec::new());
    let crisp = tallest_glyph(&harness.frame(Vec::new()));
    let moved = |harness: &mut Harness, origin, scale| {
        view.set(Some(CanvasView::new(origin, scale)));
        harness.frame(Vec::new());
        let work = harness.document().performance().latest.work;
        (work.placed, work.painted_nodes)
    };
    let panned = moved(&mut harness, pos2(30.0, 20.0), 1.0);
    assert_eq!(panned, (1, 1), "a pan lays out and paints nothing again");
    let zoomed = moved(&mut harness, pos2(30.0, 20.0), 2.0);
    assert_eq!(zoomed, (1, 1), "a zoom lays out and paints nothing again");
    harness.advance(Duration::from_millis(200));
    let settled = harness.frame(Vec::new());
    assert!(
        tallest_glyph(&settled) >= crisp * 2 - 1,
        "once the zoom settles, the text is shaped for the scale it is shown at"
    );
    let settled = moved(&mut harness, pos2(10.0, 20.0), 2.0);
    assert_eq!(
        settled,
        (1, 1),
        "a pan after the zoom settles lays out nothing again"
    );
}

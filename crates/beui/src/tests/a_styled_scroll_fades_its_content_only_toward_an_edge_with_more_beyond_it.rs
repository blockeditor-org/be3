use super::*;
use crate::reactive::{
    ForEach, NodeRef, build, create_memo, create_signal, view, with_reactive_scope,
};
use crate::styled::Scroll;
use crate::styled::theme::SCROLL_FADE;
use beui_core::display::{Display, Layer};
use beui_core::fade::Fade;

#[test]
fn a_styled_scroll_fades_its_content_only_toward_an_edge_with_more_beyond_it() {
    let (rows, set_rows) = create_signal(2_usize);
    let scroll = NodeRef::new();
    let document = build({
        let scroll = scroll.clone();
        move || {
            let keys = create_memo(move || (0..rows.get()).collect::<Vec<usize>>());
            view! {
                <Scroll @node_ref=&scroll>
                    <ForEach keys>
                        {|_: usize| view! {
                            <Frame height=40.0>
                                <Spacer />
                            </Frame>
                        }}
                    </ForEach>
                </Scroll>
            }
        }
    });
    let scroll = scroll.get();
    let mut harness = Harness::new(document);
    let fades = |output: &crate::FrameOutput| {
        let mut fades = Vec::new();
        for layer in output.layers.iter() {
            if let Layer::Display { display, .. } = layer {
                collect(display, &mut fades);
            }
        }
        fades
    };

    let output = harness.frame(Vec::new());
    assert_eq!(
        fades(&output),
        Vec::<[f32; 4]>::new(),
        "content that fits does not fade"
    );

    with_reactive_scope(harness.document_mut(), move || set_rows.set(40));
    let output = harness.frame(Vec::new());
    assert!(!fades(&output).is_empty());
    assert!(
        fades(&output)
            .iter()
            .all(|widths| *widths == [0.0, 0.0, 0.0, SCROLL_FADE]),
        "only the bottom fades once more content lies below: {:?}",
        fades(&output)
    );

    harness.document_mut().set_scroll_offset(scroll, 10.0);
    let output = harness.frame(Vec::new());
    assert!(
        fades(&output)
            .iter()
            .all(|widths| *widths == [0.0, 10.0, 0.0, SCROLL_FADE]),
        "the top fades in as far as the content has scrolled past it: {:?}",
        fades(&output)
    );

    harness.document_mut().set_scroll_offset(scroll, 10_000.0);
    let output = harness.frame(Vec::new());
    assert!(
        fades(&output)
            .iter()
            .all(|widths| *widths == [0.0, SCROLL_FADE, 0.0, 0.0]),
        "scrolled to the end, only the top fades: {:?}",
        fades(&output)
    );
}

fn collect(display: &Display, fades: &mut Vec<[f32; 4]>) {
    for (_, entry, child) in display.children() {
        if entry.fade != Fade::NONE {
            fades.push(entry.fade.widths);
        }
        collect(child, fades);
    }
}

use super::*;
use crate::primitives::Frame;
use crate::reactive::*;
use crate::styled::Scroll;
use crate::styled::theme::SCROLL_FADE;
use crate::styled::*;
use beui_core::display::{Group, Layer, Part};

#[test]
fn a_styled_scroll_fades_its_content_only_toward_an_edge_with_more_beyond_it() {
    let _root = crate::reactive::RootScope::new();
    let rows = create_signal(2_usize);
    let scroll = NodeRef::new();
    let document = build_node(move || {
        let keys = create_memo(move || (0..rows.get()).collect::<Vec<usize>>());
        view! {
            <Scroll @node_ref=&scroll>
                <ForEach keys>|_: usize|
                    <Frame height=40.0>
                        <Spacer />
                    </Frame>
                </ForEach>
            </Scroll>
        }
    });
    let scroll = scroll.get();
    let mut harness = Harness::new(document);
    let fades = |output: &crate::native::FrameOutput| {
        let mut fades = Vec::new();
        for layer in output.main().layers.iter() {
            if let Layer::Display(_, display) = layer {
                collect(&display.parts, &mut fades);
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

    rows.set(40);
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

fn collect(parts: &[Part], fades: &mut Vec<[f32; 4]>) {
    for part in parts {
        match part {
            Part::Shapes { .. } => {}
            Part::Child(_, _, child) => collect(&child.parts, fades),
            Part::Group(group, inner) => {
                if let Group::Mask { fade, .. } = group
                    && !fade.is_none()
                {
                    fades.push(fade.widths);
                }
                collect(inner, fades);
            }
        }
    }
}

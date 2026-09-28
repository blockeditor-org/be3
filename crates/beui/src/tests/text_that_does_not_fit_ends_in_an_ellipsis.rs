use super::*;
use crate::reactive::{Direction, ItemSize, List, view};

const LONG: &str = "Checked out a commit whose summary is far too long for the toolbar";
const SHORT: &str = "Checked out";

#[test]
fn text_that_does_not_fit_ends_in_an_ellipsis() {
    let document = build(|| {
        view! {
            <List direction=Direction::Horizontal spacing=0.0>
                <Frame width=100.0 height=20.0 />
                <Text @sizing=ItemSize::Percent(100.0) string=LONG ellipsis=true />
                <Text string=SHORT ellipsis=true />
            </List>
        }
    });
    let mut harness = Harness::sized(document, vec2(300.0, 40.0));
    let output = harness.frame(Vec::new());
    let texts: Vec<_> = output
        .shapes()
        .iter()
        .filter_map(|shape| match shape {
            crate::Shape::Text { origin, galley, .. } => Some((*origin, galley.clone())),
            _ => None,
        })
        .collect();
    let [(origin, truncated), (short_origin, short)] = texts.as_slice() else {
        panic!("expected two texts, painted {}", texts.len());
    };

    let kept = truncated
        .text()
        .strip_suffix('\u{2026}')
        .expect("the text that does not fit ends in an ellipsis");
    assert!(!kept.is_empty() && LONG.starts_with(kept));
    assert!(origin.x >= 100.0);
    assert!(
        origin.x + truncated.size().x <= short_origin.x,
        "the truncated text stops short of the text after it"
    );
    assert_eq!(short.text(), SHORT, "text that fits is left whole");
    assert!(short_origin.x + short.size().x <= 300.0);
}

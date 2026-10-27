use super::*;
use crate::FontId;
use crate::primitives::*;
use crate::reactive::*;

#[test]
fn a_baseline_row_lines_up_text_of_different_sizes() {
    let document = build_node(|| {
        view! {
            <List direction=Direction::Horizontal align=Align::Baseline spacing=8.0>
                <Text font={FontId::proportional(12.0)}>"Small"</Text>
                <Frame padding={Sides::new(0.0, 6.0, 0.0, 0.0)}>
                    <Text font={FontId::proportional(28.0)}>"Large"</Text>
                </Frame>
            </List>
        }
    });
    let mut harness = Harness::sized(document, vec2(300.0, 80.0));
    let output = harness.frame(Vec::new());
    let lines: Vec<(f32, f32)> = output
        .main()
        .shapes()
        .iter()
        .filter_map(|shape| match shape {
            crate::native::Shape::Text { origin, galley, .. } => {
                let top = galley.lines().first().map_or(0.0, |line| line.top);
                Some((origin.y, origin.y + top + galley.baseline()))
            }
            _ => None,
        })
        .collect();
    let [(small_top, small), (large_top, large)] = lines[..] else {
        panic!("expected two runs of text, got {lines:?}");
    };
    assert!(
        (small - large).abs() <= 0.5,
        "baselines {small} and {large} differ"
    );
    assert!(
        small_top > large_top,
        "the small text sits lower to line up"
    );
}

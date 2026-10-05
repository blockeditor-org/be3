use super::*;

use crate::reactive::{ForEach, List, Text, build, view};
use crate::unstyled::Scroll;

#[test]
fn slowing_repaints_still_copies_what_a_scroll_moved_at_once() {
    let document = build(|| {
        view! {
            <Frame color=Color32::BLACK radius=0>
                <List spacing=0.0>
                    <Frame height=200.0>
                        <Scroll @test_id="scroll">
                            <ForEach keys={(0..30).collect::<Vec<usize>>()}>
                                {|row: usize| view! {
                                    <Frame height=40.0>
                                        <Text string={format!("row {row}")} />
                                    </Frame>
                                }}
                            </ForEach>
                        </Scroll>
                    </Frame>
                </List>
            </Frame>
        }
    });
    let mut harness = Harness::sized(document, WIDE_VIEWPORT);
    harness.toggle_inspector();
    harness.click(harness.performance_tab_center());
    harness.frame(Vec::new());
    harness.click(harness.slow_repaint_toggle_center());
    harness.settle();
    let scroll = harness.find("scroll");
    let viewport = harness.rect(scroll);
    harness.frame(vec![Event::PointerMoved(viewport.center())]);
    harness.settle();

    let output = harness.frame(vec![Event::Scroll(vec2(0.0, -40.0))]);
    let moved = output
        .moved()
        .expect("a slowed repaint still copies what the scroll showed");
    assert_eq!(moved.by, vec2(0.0, -40.0));
    let exposed = Rect::from_min_max(pos2(viewport.left(), moved.to().bottom()), viewport.max);
    let mut scanned: Vec<Rect> = output
        .damage
        .rects()
        .iter()
        .map(|rect| rect.intersect(exposed))
        .filter(Rect::is_positive)
        .collect();
    assert!(scanned.iter().all(|band| band.height() < exposed.height()));
    loop {
        let output = harness.frame(Vec::new());
        scanned.extend(
            output
                .damage
                .rects()
                .iter()
                .map(|rect| rect.intersect(exposed))
                .filter(Rect::is_positive),
        );
        if !output.repaint {
            break;
        }
        assert!(
            scanned.len() < 600,
            "the exposed rows never finished scanning in"
        );
    }

    let covered = scanned
        .iter()
        .fold(Rect::NOTHING, |covered, band| covered.union(*band));
    assert_eq!(covered, exposed);
}

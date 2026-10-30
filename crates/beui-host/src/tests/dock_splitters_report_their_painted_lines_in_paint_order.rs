use super::*;
use dock::unstyled::{DockPane, DockSplit, DockTab, DockingLayout};

#[test]
fn dock_splitters_report_their_painted_lines_in_paint_order() {
    let _root = crate::reactive::RootScope::new();
    let document = build_node(move || {
        let layout = DockingLayout::<u64>::new();
        view! {
            <dock::styled::Docking layout>
                <DockSplit id="a" fraction=0.2>
                    <DockPane id="one">
                        <DockTab id=1u64 title="One">
                            <Embed slot={EmbedSlot::new()} />
                        </DockTab>
                    </DockPane>
                    <DockSplit id="b" fraction=0.25>
                        <DockPane id="two">
                            <DockTab id=2u64 title="Two">
                                <Embed slot={EmbedSlot::new()} />
                            </DockTab>
                        </DockPane>
                        <DockSplit id="c" fraction=0.33>
                            <DockPane id="three">
                                <DockTab id=3u64 title="Three">
                                    <Embed slot={EmbedSlot::new()} />
                                </DockTab>
                            </DockPane>
                            <DockSplit id="d" fraction=0.5>
                                <DockPane id="four">
                                    <DockTab id=4u64 title="Four">
                                        <Embed slot={EmbedSlot::new()} />
                                    </DockTab>
                                </DockPane>
                                <DockPane id="five">
                                    <DockTab id=5u64 title="Five">
                                        <Embed slot={EmbedSlot::new()} />
                                    </DockTab>
                                </DockPane>
                            </DockSplit>
                        </DockSplit>
                    </DockSplit>
                </DockSplit>
            </dock::styled::Docking>
        }
    });
    let mut harness = Harness::sized(document, vec2(1000.0, 600.0));
    harness.frame(Vec::new());
    let window = crate::WindowId::MAIN;
    let mut splitters: Vec<_> = ["a", "b", "c", "d"]
        .into_iter()
        .map(|id| harness.find(&format!("dock.splitter.{id}")))
        .map(|node| (harness.document().paint_order(node), harness.rect(node)))
        .collect();
    splitters.sort_by(|(left, _), (right, _)| left.cmp(right));
    let reaches: Vec<Rect> = splitters.into_iter().map(|(_, rect)| rect).collect();
    let covers: Vec<Rect> = harness
        .document()
        .overlay_layers(window)
        .into_iter()
        .map(|(_, rect)| rect)
        .filter(|rect| reaches.contains(rect))
        .collect();
    assert_eq!(covers, reaches);
    let lines: Vec<Rect> = harness.document().overlay_painted_rects(window);
    let owners: Vec<usize> = lines
        .iter()
        .filter_map(|line| reaches.iter().position(|reach| reach.intersects(*line)))
        .collect();
    assert_eq!(
        owners,
        (0..reaches.len()).collect::<Vec<_>>(),
        "the splitters' lines come in paint order, whatever order a hash map holds them in, so a painting of the window is the same on every run: {lines:?}"
    );
}

use super::*;
use dock::unstyled::{DockPane, DockSplit, DockTab, DockingLayout};

#[test]
fn a_window_built_twice_paints_and_reports_the_same() {
    let built = || {
        let _root = crate::reactive::RootScope::new();
        let document = build_node(move || {
            let layout = DockingLayout::<u64>::new();
            view! {
                <dock::styled::Docking layout drag_modifier={Some(Modifiers::LOGO)}>
                    <DockSplit id="a" fraction=0.2>
                        <DockPane id="one">
                            <DockTab id=1u64 title="One">
                                <Embed slot={EmbedSlot::new()} />
                            </DockTab>
                            <DockTab id=6u64 title="Six">
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
        let document = harness.document();
        let reported = format!(
            "claims {:?}\ndrawing {:?}\nclear {:?}\noverlay {:?}\npainted {:?}",
            document.press_claims(window),
            document.drawing_rects(window),
            document.kept_clear(window),
            document.overlay_layers(window),
            document.overlay_painted_rects(window),
        );
        let asked: Vec<String> = ["tree", "ids", "comps", "actions"]
            .into_iter()
            .map(|word| harness.ask(&[word]).unwrap_or_else(|error| error))
            .collect();
        (harness.painting(), reported, asked)
    };
    let (painted, reported, asked) = built();
    for _ in 0..4 {
        let (again, reported_again, asked_again) = built();
        assert_eq!(
            reported_again, reported,
            "the window reports its rects in the same order every time it is built"
        );
        assert_eq!(
            asked_again, asked,
            "the window describes itself the same way every time it is built"
        );
        assert!(
            again == painted,
            "the window paints the same every time it is built"
        );
    }
}

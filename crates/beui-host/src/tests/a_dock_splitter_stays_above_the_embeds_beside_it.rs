use super::*;
use crate::primitives::*;
use crate::reactive::*;
use dock::unstyled::{DockPane, DockSplit, DockTab, DockingLayout};

#[test]
fn a_dock_splitter_stays_above_the_embeds_beside_it() {
    let _root = crate::reactive::RootScope::new();
    let document = build_node(move || {
        let layout = DockingLayout::<u64>::new();
        view! {
            <dock::styled::Docking layout>
                <DockSplit id="root" fraction=0.5>
                    <DockPane id="left">
                        <DockTab id=1u64 title="Left">
                            <Embed slot={EmbedSlot::new()} @test_id="embed" />
                        </DockTab>
                    </DockPane>
                    <DockPane id="right">
                        <DockTab id=2u64 title="Right">
                            <Frame />
                        </DockTab>
                    </DockPane>
                </DockSplit>
            </dock::styled::Docking>
        }
    });
    let mut harness = Harness::sized(document, vec2(1000.0, 600.0));
    harness.frame(Vec::new());
    let splitter = harness.rect(harness.find("dock.splitter.root"));
    let embed = harness.find("embed");
    let window = crate::WindowId::MAIN;
    let embed_order = harness.document().paint_order(embed);
    let covers = harness.document().overlay_layers(window);
    assert!(
        covers
            .iter()
            .any(|(order, rect)| *rect == splitter && *order > embed_order),
        "the splitter's reach covers the embed painted before it, for input: {covers:?}, splitter {splitter:?}"
    );
    let line = |harness: &Harness| {
        harness
            .document()
            .overlay_painted_rects(window)
            .into_iter()
            .find(|rect| rect.intersects(splitter))
            .expect("the splitter's line is painted over the embeds")
    };
    assert_eq!(line(&harness).width(), 1.0);
    assert_eq!(line(&harness).center().x, splitter.center().x);

    harness.frame(vec![Input::PointerMoved(splitter.center())]);
    harness.frame(Vec::new());
    assert_eq!(
        line(&harness).width(),
        3.0,
        "a hovered splitter's wider line is drawn over the embeds on both sides"
    );
}

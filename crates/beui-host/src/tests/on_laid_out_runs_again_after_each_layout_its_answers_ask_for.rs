use super::*;
use crate::primitives::*;
use crate::reactive::*;

#[test]
fn on_laid_out_runs_again_after_each_layout_its_answers_ask_for() {
    let _root = crate::reactive::RootScope::new();
    let first = EmbedSlot::new();
    let second = EmbedSlot::new();
    let tall = create_signal(10.0_f32);
    let wide = create_signal(10.0_f32);
    let mut document = build_node(move || {
        view! {
            <List spacing=0.0>
                <Dynamic value={tall}>|tall: f32|
                    <Frame height=tall>
                        <Embed slot={first} />
                    </Frame>
                </Dynamic>
                <Dynamic value={wide}>|wide: f32|
                    <Frame width=wide height=10.0>
                        <Embed slot={second} />
                    </Frame>
                </Dynamic>
            </List>
        }
    });
    let runs = Rc::new(std::cell::Cell::new(0));
    let counted = Rc::clone(&runs);
    document.on_laid_out(move || {
        counted.set(counted.get() + 1);
        if first
            .placement()
            .is_some_and(|placement| placement.rect.height() == 10.0)
        {
            tall.set(40.0);
        }
        if second
            .placement()
            .is_some_and(|placement| placement.rect.min.y == 40.0)
        {
            wide.set(80.0);
        }
    });
    let mut harness = Harness::new(document);
    harness.frame(Vec::new());

    assert_eq!(
        second.placement().map(|placement| placement.rect.width()),
        Some(80.0),
        "what the hook answers to the layout its first answer asked for is laid out in the same frame"
    );
    assert_eq!(
        runs.get(),
        3,
        "the hook runs after the first layout and again after each layout an answer asked for"
    );
}

use super::*;
use crate::primitives::*;
use crate::reactive::*;

#[test]
fn what_on_laid_out_changes_is_laid_out_before_the_frame_is_painted() {
    let _root = crate::reactive::RootScope::new();
    let measured = EmbedSlot::new();
    let answered = EmbedSlot::new();
    let (height, set_height) = create_signal(10.0_f32);
    let (first, second) = (measured, answered);
    let mut document = build(move || {
        view! {
            <List spacing=0.0>
                <Frame height=50.0>
                    <Embed slot={first} />
                </Frame>
                <Dynamic value={height}>
                    |height: f32| {
                        let slot = second;
                        view! {
                            <Frame height=height>
                                <Embed slot={slot} />
                            </Frame>
                        }
                    }
                </Dynamic>
            </List>
        }
    });
    document.on_laid_out(move || {
        if let Some(placement) = measured.placement() {
            set_height.set(placement.rect.width() / 4.0);
        }
    });
    let mut harness = Harness::new(document);
    harness.frame(Vec::new());

    let width = measured.placement().map(|placement| placement.rect.width());
    assert_eq!(
        answered
            .placement()
            .map(|placement| placement.rect.height()),
        width.map(|width| width / 4.0),
        "the answer to the first layout is laid out in the frame that asked"
    );
}

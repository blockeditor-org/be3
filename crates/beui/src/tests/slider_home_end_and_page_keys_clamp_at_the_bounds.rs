use super::*;
use crate::reactive::*;
use crate::styled::*;

#[test]
fn slider_home_end_and_page_keys_clamp_at_the_bounds() {
    let (document, [slider]) = toolbar_of(|| {
        [view! {
            <Slider value=0.5 />
        }]
    });
    let mut harness = Harness::new(document);
    harness.key(Key::Tab, Modifiers::NONE);
    harness.key(Key::End, Modifiers::NONE);
    harness.key(Key::PageUp, Modifiers::NONE);
    assert_eq!(
        harness
            .document()
            .api::<styled::SliderApi>(slider)
            .value()
            .get(),
        1.0
    );
    harness.key(Key::PageDown, Modifiers::NONE);
    assert!(
        (harness
            .document()
            .api::<styled::SliderApi>(slider)
            .value()
            .get()
            - 0.8)
            .abs()
            < 0.001
    );
    harness.key(Key::Home, Modifiers::NONE);
    harness.key(Key::ArrowDown, Modifiers::NONE);
    assert_eq!(
        harness
            .document()
            .api::<styled::SliderApi>(slider)
            .value()
            .get(),
        0.0
    );
}

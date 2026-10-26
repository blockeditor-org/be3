use super::*;
use beui::{PreferenceLayer, PreferenceSource};
use std::time::Duration;

#[test]
fn instant_indicators_hold_still_and_resume() {
    let document = build(move || {
        view! {
            <Root>
                <Frame fill=ColorRole::Background padding=Space::Xl>
                    <List spacing=Space::Lg>
                        <Progress label="Loading" />
                        <TextInput @test_id="instant.input" value="A visible caret" />
                    </List>
                </Frame>
            </Root>
        }
    });
    let mut test = DocumentTest::new(document, Vec2::new(400.0, 200.0));
    test.click("instant.input");
    test.context().set_preference_layer(
        PreferenceSource::System,
        PreferenceLayer {
            motion: Some(Motion::Instant),
            ..PreferenceLayer::EMPTY
        },
    );
    test.frame(Vec::new());
    test.snapshot("instant_indicators");
    test.advance(Duration::from_secs(2));
    test.frame(Vec::new());
    test.snapshot("instant_indicators_after_waiting");
    test.context().set_preference_layer(
        PreferenceSource::System,
        PreferenceLayer {
            motion: Some(Motion::Animated),
            ..PreferenceLayer::EMPTY
        },
    );
    test.advance(Duration::from_millis(100));
    test.frame(Vec::new());
    test.snapshot("indicators_resumed");
}

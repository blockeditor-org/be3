use super::*;
use crate::reactive::*;
use crate::styled::use_theme;
use crate::{PreferenceLayer, PreferenceSource, ThemeKind};

#[test]
fn the_theme_follows_the_preferences_after_the_first_component_that_asked_for_it_goes() {
    let _root = RootScope::new();
    let shown = create_signal(true);
    let document = build_node(move || {
        view! {
            <List spacing=0.0>
                <Show condition={shown}>|| {
                    let theme = use_theme();
                    view! {
                        <Frame height=10.0 fill={theme.surface.get()} />
                    }
                }</Show>
            </List>
        }
    });
    let mut harness = Harness::new(document);
    harness.frame(Vec::new());
    with_installed(harness.document_mut(), |_| shown.set(false));
    harness.frame(Vec::new());

    for kind in [ThemeKind::Dark, ThemeKind::Light] {
        harness.context().set_preference_layer(
            PreferenceSource::System,
            PreferenceLayer {
                theme: Some(kind),
                ..PreferenceLayer::EMPTY
            },
        );
        harness.frame(Vec::new());
        let seen = crate::native::with_reactive_scope(harness.document_mut(), || {
            use_theme().kind.get_untracked()
        });
        assert_eq!(seen, kind);
    }
}

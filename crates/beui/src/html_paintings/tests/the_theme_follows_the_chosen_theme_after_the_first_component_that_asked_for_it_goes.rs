use super::*;
use crate::ThemeKind;

#[test]
fn the_theme_follows_the_chosen_theme_after_the_first_component_that_asked_for_it_goes() {
    let first = Scope::detached();
    first.run(|| {
        let _ = use_theme();
    });
    first.dispose();
    flush();

    let later = Scope::detached();
    for kind in [ThemeKind::Dark, ThemeKind::Light, ThemeKind::Dark] {
        crate::html::choose_theme(Some(kind));
        flush();
        assert_eq!(later.run(|| use_theme().kind.get_untracked()), kind);
    }
}

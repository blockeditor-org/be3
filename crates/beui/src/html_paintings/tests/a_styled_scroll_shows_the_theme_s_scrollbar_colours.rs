use super::*;
use crate::ThemeKind;
use beui_macros::view;

fn scroll() -> crate::primitives::Root {
    view! {
        <Root>
            <Frame height=Size::Custom(160.0)>
                <Scroll>
                    <List spacing=Space::None>
                        <ForEach keys={(1..=20).collect::<Vec<usize>>()}>|row: usize|
                            <ListRow label={format!("Row {row}")} />
                        </ForEach>
                    </List>
                </Scroll>
            </Frame>
        </Root>
    }
}

#[test]
fn a_styled_scroll_shows_the_theme_s_scrollbar_colours() {
    let markup = crate::html::render_to_string(scroll);
    for expected in ["scrollbar-width: thin", "scrollbar-color: "] {
        assert!(markup.contains(expected), "{expected} in {markup}");
    }
    for (kind, name) in [
        (ThemeKind::Light, "html_styled_scroll_light.browser"),
        (ThemeKind::Dark, "html_styled_scroll_dark.browser"),
    ] {
        crate::html::choose_theme(Some(kind));
        flush();
        painting([240, 160], scroll).snapshot(name);
    }
    crate::html::choose_theme(None);
    flush();
}

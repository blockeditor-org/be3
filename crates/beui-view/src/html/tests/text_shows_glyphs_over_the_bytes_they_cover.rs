use super::*;

#[test]
fn text_shows_glyphs_over_the_bytes_they_cover() {
    painting([320, 140], || {
        let glyphs = vec![
            TextGlyph::new(3..4, "\u{b7}", FontId::monospace(16.0), BLUE),
            TextGlyph::new(7..8, "\u{2192}", FontId::monospace(16.0), BLUE),
        ];
        let wraps = create_signal(true);
        view! {
            <Root clear_color={PAPER}>
                <List spacing={12.0}>
                    <Text color={INK} font={FontId::monospace(16.0)} glyphs={glyphs}>
                        "one two\tthree"
                    </Text>
                    <Frame width={120.0}>
                        <Text color={INK}>
                            <Span break_after={wraps}>"unbreakable"</Span>
                            <Span>"words"</Span>
                        </Text>
                    </Frame>
                </List>
            </Root>
        }
    })
    .snapshot("html_text_glyphs.browser");
}

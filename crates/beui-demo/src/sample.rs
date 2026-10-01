use super::*;

#[component]
pub(crate) fn ScrollPage(children: Children<ListChild>) -> NodeId {
    view! {
        <Scroll>
            <Frame padding_horizontal=PAGE_PADDING padding_vertical=PAGE_PADDING>
                <List spacing=PAGE_SPACING children />
            </Frame>
        </Scroll>
    }
}

#[component]
pub(crate) fn Sample(title: &'static str, code: Vec<&'static str>, children: Child) -> NodeId {
    let (open, set_open) = create_signal(false);
    let pressed = open.clone();
    let source = code.join("\n\n");
    view! {
        <List spacing=SECTION_SPACING>
            <List direction=Direction::Horizontal align=Align::Center spacing=8.0>
                <Heading @sizing=ItemSize::Percent(100.0) content={title} />
                <ToggleButton
                    @test_id={format!("demo.code.{title}")}
                    label="Code"
                    glyph=ICON_CODE
                    pressed={pressed}
                    on_change={move |shown| set_open.set(shown)}
                />
            </List>
            {children}
            <Show condition={open}>
                <CodeBlock source={source} />
            </Show>
        </List>
    }
}

#[component]
fn CodeBlock(source: String) -> NodeId {
    let theme = use_theme();
    let spans = create_memo(clone!(source theme -> move || {
        let theme = theme.get();
        code_spans(&source, theme.background, theme.text)
    }));
    view! {
        <Frame
            color={theme.surface.clone()}
            outline={theme.border.clone()}
            outline_width=1.0
            outline_visible=true
            radius=RADIUS
            padding_horizontal=CODE_PADDING
            padding_vertical=CODE_PADDING
        >
            <SelectableText @test_id="demo.code" child_size=ItemSize::Percent(100.0)>
                <Text
                    string={source}
                    spans={spans}
                    font_size=FONT_SMALL
                    color={theme.text.clone()}
                    monospace=true
                    wrap=true
                />
            </SelectableText>
        </Frame>
    }
}

pub(crate) fn code_spans(source: &str, background: Color32, text: Color32) -> Vec<TextSpan> {
    let [red, green, blue, _] = background.to_array();
    let dark = u32::from(red) + u32::from(green) + u32::from(blue) < DARK_BACKGROUND;
    let colors = match dark {
        true => SyntaxColors::DEFAULT,
        false => SyntaxColors::uniform(text),
    };
    let document = Arc::new(TextBuffer::new(source)) as Arc<dyn text_editor_core::Document>;
    let highlight = Highlighter::new(document, Language::Rust).highlight();
    let font = FontId::monospace(FONT_SMALL);
    let mut spans: Vec<TextSpan> = Vec::new();
    for (start, character) in source.char_indices() {
        let color = colors.scope(highlight.advance_and_read(start));
        let end = start + character.len_utf8();
        match spans.last_mut() {
            Some(span) if span.style.color == color => span.range.end = end,
            _ => spans.push(TextSpan::text(start..end, SpanStyle::new(font, color))),
        }
    }
    spans
}

use super::*;
use beui::Align;
use beui::Color;

#[component]
pub(crate) fn ScrollPage(children: Children<ListChild>) -> NodeId {
    view! {
        <Scroll>
            <Frame padding=PAGE_PADDING>
                <List spacing=PAGE_SPACING children />
            </Frame>
        </Scroll>
    }
}

#[component]
pub(crate) fn Sample(title: &'static str, code: Vec<&'static str>, children: Child) -> NodeId {
    let open = create_signal(false);
    let source = code.join("\n\n");
    view! {
        <List spacing=SECTION_SPACING>
            <List direction=Direction::Horizontal align=Align::Center spacing=Space::Md>
                <Text style=TextStyle::HEADING @sizing=ItemSize::Percent(100.0)>{title}</Text>
                <ToggleButton
                    @test_id={format!("demo.code.{title}")}
                    label="Code"
                    glyph=ICON_CODE
                    pressed={open}
                />
            </List>
            {children}
            <Show condition={open}>
                <CodeBlock source={source.clone()} />
            </Show>
        </List>
    }
}

#[component]
fn CodeBlock(source: String) -> NodeId {
    let theme = use_theme();
    let syntax = theme.syntax;
    let tokens = create_memo({
        let source = source.clone();
        move || code_tokens(&source, syntax.get())
    });
    view! {
        <Frame
            fill=ColorRole::Surface
            border={Border::new(ColorRole::Border).width(Stroke::Hairline)}
            radius=Radius::Md
            padding=CODE_PADDING
        >
            <SelectableText @test_id="demo.code" child_size=ItemSize::Percent(100.0)>
                <Text size=TextSize::Sm family=Family::Mono>
                    <ForEach keys={tokens}>|(start, end, color): (usize, usize, Color)| {
                        let piece = source[start..end].to_owned(); view! {
                            <Span color={TextColor::Custom(color)}>{piece}</Span>
                        }
                    }</ForEach>
                </Text>
            </SelectableText>
        </Frame>
    }
}

pub(crate) fn code_tokens(source: &str, colors: SyntaxColors) -> Vec<(usize, usize, Color)> {
    let document = Arc::new(TextBuffer::new(source)) as Arc<dyn text_editor_core::Document>;
    let highlight = Highlighter::new(document, Language::Rust).highlight();
    let mut tokens: Vec<(usize, usize, Color)> = Vec::new();
    for (start, character) in source.char_indices() {
        let color = colors.scope(highlight.advance_and_read(start));
        let end = start + character.len_utf8();
        match tokens.last_mut() {
            Some(token) if token.2 == color => token.1 = end,
            _ => tokens.push((start, end, color)),
        }
    }
    tokens
}

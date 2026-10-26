use super::*;

#[test]
fn text_wraps_cuts_and_styles_its_spans() {
    painting([320, 260], || {
        view! {
            <Root clear_color={PAPER}>
                <List spacing={12.0}>
                    <Text color={INK}>
                        "A paragraph long enough to wrap onto a second line in a narrow window."
                    </Text>
                    <Text color={INK} overflow={Overflow::Ellipsis}>
                        "A line too long for its box is cut with an ellipsis at its end."
                    </Text>
                    <Text
                        color={INK}
                        align={Some(TextAlign::Center)}
                        font={FontId::proportional(18.0)}
                    >
                        "Centred and larger"
                    </Text>
                    <Text color={INK}>
                        "Plain, "
                        <Span bold={Some(true)}>"bold"</Span>
                        ", "
                        <Span italic={Some(true)} color={Some(BLUE)}>"blue italic"</Span>
                        ", "
                        <Span underline={Some(true)}>"underlined"</Span>
                        <Gap width={24.0} />
                        <Span
                            background={Some(SpanBackground { color: AMBER, radius: 3.0, outset: Vec2::ZERO })}
                        >
                            "marked"
                        </Span>
                        <Break />
                        "and a line after a break."
                    </Text>
                    <Text
                        color={INK}
                        overflow={Overflow::Wrap { at: WrapAt::Words, max_lines: Some(2) }}
                    >
                        "At most two lines are shown of this text, which would otherwise run on to a third and a fourth line in this box."
                    </Text>
                </List>
            </Root>
        }
    })
    .snapshot("html_text.browser");
}

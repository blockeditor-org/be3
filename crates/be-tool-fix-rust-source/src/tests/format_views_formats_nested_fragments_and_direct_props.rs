#[test]
fn format_views_formats_nested_fragments_and_direct_props() {
    let source = br#"fn main() {
    view! { <><Panel heading=<Heading>"Title"</Heading> blank=<></>><><Show condition=true><><Span>"One"</Span><Span>"Two"</Span></></Show></></Panel></> }
}
"#;
    let expected = br#"fn main() {
    view! {
        <>
            <Panel heading=<Heading>"Title"</Heading> blank=<></>>
                <>
                    <Show condition=true>
                        <>
                            <Span>"One"</Span>
                            <Span>"Two"</Span>
                        </>
                    </Show>
                </>
            </Panel>
        </>
    }
}
"#;
    let formatted = crate::views::format_views(source).unwrap();
    assert_eq!(
        String::from_utf8_lossy(&formatted),
        String::from_utf8_lossy(expected)
    );
    assert_eq!(crate::views::format_views(&formatted).unwrap(), formatted);
}

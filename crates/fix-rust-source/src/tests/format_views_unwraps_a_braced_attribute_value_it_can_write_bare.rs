use super::*;

#[test]
fn format_views_unwraps_a_braced_attribute_value_it_can_write_bare() {
    let source = r#"fn build() -> NodeId {
    view! {
        <abc def={"ghi"} @test_id={"mjk"} count={-1} on={true} align={TextAlign::Start} />
        <Frame color={theme::surface()} outline={&border} hidden={!open} label={ label } />
    }
}
"#;

    assert_eq!(
        formatted(source),
        r#"fn build() -> NodeId {
    view! {
        <abc def="ghi" @test_id="mjk" count=-1 on=true align=TextAlign::Start />
        <Frame color=theme::surface() outline=&border hidden=!open label=label />
    }
}
"#
    );
}

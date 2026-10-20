use super::*;

#[test]
fn a_qualified_modifier_can_follow_a_value_child() {
    let view =
        syn::parse2::<View>(quote! { <List>{content} @custom::decorate="row"</List> }).unwrap();
    let expanded = expand_view(&view).to_string();
    assert!(expanded.contains(r#"custom :: decorate (content , "row")"#));
}

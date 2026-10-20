use super::*;

#[test]
fn repeated_modifiers_compose_in_source_order() {
    let view = syn::parse2::<View>(quote! { <Text @decorate="one" @decorate="two" /> }).unwrap();
    let expanded = expand_view(&view).to_string();
    assert!(expanded.contains(r#"decorate (decorate (Text () . build () , "one") , "two")"#));
}

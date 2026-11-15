use super::*;

#[test]
fn a_type_names_a_parameter_bare_only_outside_a_projection() {
    let param: Ident = syn::parse_quote!(V);
    let names = |ty: Type| mentions_bare(&ty, &param);
    assert!(names(syn::parse_quote!(V)));
    assert!(names(syn::parse_quote!(Vec<V>)));
    assert!(names(syn::parse_quote!(Children<CaseOf<V, C>>)));
    assert!(names(syn::parse_quote!((V, usize))));
    assert!(!names(syn::parse_quote!(V::Handed)));
    assert!(!names(syn::parse_quote!(RenderFn<V::Handed, B>)));
    assert!(!names(syn::parse_quote!(
        Children<CaseOf<<V as Matched>::Value, C>>
    )));
    assert!(!names(syn::parse_quote!(Vec<Value>)));
}

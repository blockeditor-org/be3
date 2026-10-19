use super::*;

#[test]
fn component_valued_props_validate_nested_views() {
    assert!(
        syn::parse2::<View>(quote! { <Panel decoration=[<Title /> "text" {extra}] /> }).is_ok()
    );
    assert!(syn::parse2::<View>(quote! { <Panel decoration=[] /> }).is_ok());
    let error = syn::parse2::<View>(quote! { <Panel decoration=[<Title></Other>] /> })
        .err()
        .unwrap();
    assert!(error.to_string().contains("mismatched closing tag"));
    let error = syn::parse2::<View>(quote! { <Panel decoration=[<Title text="a" text="b" />] /> })
        .err()
        .unwrap();
    assert!(error.to_string().contains("set more than once"));
}

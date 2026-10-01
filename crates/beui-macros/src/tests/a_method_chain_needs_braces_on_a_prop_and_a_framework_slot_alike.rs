use super::*;

#[test]
fn a_method_chain_needs_braces_on_a_prop_and_a_framework_slot_alike() {
    let errors = [
        syn::parse2::<View>(quote! { <Frame width=Width::default().get() /> }),
        syn::parse2::<View>(quote! { <Frame @sizing=ItemSize::Intrinsic.min(4.0) /> }),
    ]
    .map(|parsed| match parsed {
        Ok(_) => panic!("an unbraced method chain is rejected"),
        Err(error) => error.to_string(),
    });

    assert_eq!(
        errors,
        [
            "wrap a value that goes on past a literal, a path or a call in braces: `width={...}`",
            "wrap a value that goes on past a literal, a path or a call in braces: `sizing={...}`",
        ]
    );
    assert!(syn::parse2::<View>(quote! { <Frame width={Width::default().get()} /> }).is_ok());
    assert!(syn::parse2::<View>(quote! { <Frame @sizing={ItemSize::Intrinsic.min(4.0)} /> }).is_ok());
}

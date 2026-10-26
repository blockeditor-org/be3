use super::*;

#[test]
fn a_prop_written_twice_on_one_tag_is_rejected() {
    let Err(error) = syn::parse2::<View>(
        quote! { <Text overflow=Overflow::Ellipsis font_size=12.0 overflow=Overflow::Visible /> },
    ) else {
        panic!("a repeated prop is rejected");
    };

    assert_eq!(
        error.to_string(),
        "prop `overflow` is set more than once on this tag"
    );
}

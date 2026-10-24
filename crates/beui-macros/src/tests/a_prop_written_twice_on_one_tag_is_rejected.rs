use super::*;

#[test]
fn a_prop_written_twice_on_one_tag_is_rejected() {
    let Err(error) =
        syn::parse2::<View>(quote! { <Text wrap=Wrap::Words font_size=12.0 wrap=Wrap::Off /> })
    else {
        panic!("a repeated prop is rejected");
    };

    assert_eq!(
        error.to_string(),
        "prop `wrap` is set more than once on this tag"
    );
}

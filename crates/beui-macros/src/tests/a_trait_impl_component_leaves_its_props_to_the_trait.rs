use super::*;

#[test]
fn a_trait_impl_component_leaves_its_props_to_the_trait() {
    let item = syn::parse2::<syn::ItemImpl>(quote! {
        impl Theme for Plain {
            #[component]
            fn Card(#[prop(default = 1)] count: usize) -> NodeId {
                count
            }
        }
    })
    .expect("the impl parses");

    let Err(error) = associated::expand_impl(item) else {
        panic!("a prop attribute on a trait impl's component is rejected");
    };

    assert_eq!(
        error.to_string(),
        "write `#[prop(...)]` on the trait's declaration of `Card`, which owns its props"
    );
}

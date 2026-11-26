use super::*;

#[test]
fn a_props_attributes_merge_and_reject_a_key_given_twice() {
    let merged = syn::parse2::<ItemFn>(quote! {
        fn Panel(
            #[prop(children)]
            #[prop(default = Children::default())]
            content: Children<NodeId>,
        ) -> NodeId {
            0
        }
    })
    .expect("the component parses as a function");
    expand_component(merged).expect("two `#[prop(..)]` attributes merge");

    let twice = syn::parse2::<ItemFn>(quote! {
        fn Panel(#[prop(default = 1)] #[prop(default = 2)] size: u32) -> NodeId {
            0
        }
    })
    .expect("the component parses as a function");
    let Err(error) = expand_component(twice) else {
        panic!("a key given in two `#[prop(..)]` attributes is rejected");
    };
    assert_eq!(
        error.to_string(),
        "this prop already says this in a `#[prop(..)]`"
    );
}

use super::*;

#[test]
fn a_bare_children_prop_is_rejected() {
    let item = syn::parse2::<ItemFn>(quote! {
        fn Panel<B>(#[prop(children)] content: B) -> NodeId {
            0
        }
    })
    .expect("the component parses as a function");

    let Err(error) = expand_component(item) else {
        panic!("a children slot typed by a bare type parameter is rejected");
    };

    assert!(
        error.to_string().starts_with(
            "`content` takes this tag's children, so its type says which children it takes"
        ),
        "{error}"
    );
}

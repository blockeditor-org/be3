use super::*;

#[test]
fn a_children_prop_that_cannot_hold_children_is_rejected() {
    let item = syn::parse2::<ItemFn>(quote! {
        fn Listing(#[prop(children)] rows: u32) -> NodeId {
            rows
        }
    })
    .expect("the component parses as a function");

    let Err(error) = expand_component(item) else {
        panic!("a children slot that cannot hold children is rejected");
    };

    assert_eq!(
        error.to_string(),
        "prop `rows` of component `Listing` takes its children, so it must be typed `Children`, `Child`, `Option<Child>`, `Render<_>`, or `RenderFn<_>`"
    );
}

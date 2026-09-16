use super::*;

#[test]
fn a_component_marking_two_children_props_is_rejected() {
    let item = syn::parse2::<ItemFn>(quote! {
        fn TwoSlots(#[prop(children)] header: Child, #[prop(children)] body: Child) -> NodeId {
            header
        }
    })
    .expect("the component parses as a function");

    let Err(error) = expand_component(item) else {
        panic!("a second `#[prop(children)]` is rejected");
    };

    assert_eq!(
        error.to_string(),
        "component `TwoSlots` already marks another prop `#[prop(children)]`"
    );
}

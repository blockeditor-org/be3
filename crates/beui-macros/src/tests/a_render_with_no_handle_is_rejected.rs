use super::*;

#[test]
fn a_render_with_no_handle_is_rejected() {
    let item = syn::parse2::<ItemFn>(quote! {
        fn Panel(#[prop(children)] content: Option<RenderFn<(), NodeId>>) -> NodeId {
            0
        }
    })
    .expect("the component parses as a function");

    let Err(error) = expand_component(item) else {
        panic!("a render handed nothing is rejected");
    };

    assert_eq!(
        error.to_string(),
        "a render with no handle is a `BuildFn<C>`: write `BuildFn<..>` for this `RenderFn<(), ..>`"
    );
}

use super::*;

#[test]
fn a_callback_prop_with_too_many_type_arguments_is_rejected() {
    let item = syn::parse2::<ItemFn>(quote! {
        fn Picker(on_pick: Callback<u32, bool, u8>) -> NodeId {
            0
        }
    })
    .expect("the component parses as a function");

    let Err(error) = expand_component(item) else {
        panic!("a callback with three type arguments is rejected");
    };

    assert_eq!(
        error.to_string(),
        "a `Callback` prop takes one or two type arguments, as `Callback<Value>` or `Callback<Value, Result>`"
    );
}

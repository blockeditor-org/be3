use super::*;

#[test]
fn a_node_slot_written_after_a_value_child_is_rejected() {
    let Err(error) = syn::parse2::<View>(quote! { <List>{content} @test_id="row"</List> }) else {
        panic!("a framework slot that names a node is rejected after a value");
    };

    assert_eq!(
        error.to_string(),
        "`@test_id` names a node of its own, so it belongs on a tag rather than on a value \
         written between tags; only `@sizing` follows a value"
    );
}

use super::*;

#[test]
fn a_framework_slot_written_twice_on_one_tag_is_rejected() {
    let Err(error) = syn::parse2::<View>(quote! { <Text @test_id="one" @test_id="two" /> }) else {
        panic!("a repeated framework slot is rejected");
    };

    assert_eq!(
        error.to_string(),
        "`@test_id` is set more than once on this tag"
    );
}
